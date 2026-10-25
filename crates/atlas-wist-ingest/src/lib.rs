//! 消费网关 `agent-discovery` sink 落下的文件，按字节偏移增量读取。
//!
//! 为什么是「读文件」而不是「开一个写端点」：这一跳的可靠性完全由文件承担。
//! `wparse` 的 `http_sink` 只有 `max_retries = 3`，之后这一帧就没了；而文件是追加写的，
//! atlas 停机期间帧还在长。加上 `ingest_id` 里已经包含 `snap_id`（重放幂等），
//! 「读到哪了」丢了也能从 0 重来而不出错。
//!
//! 三个必须做对的细节：
//!
//! 1. **只消费完整的行**。网关可能正在写一帧（几百 KB），读到一半的半行不能解析；
//!    偏移只推进到最后一个 `\n` 之后。
//! 2. **轮转要认得出**。inode 变了或文件变短了，说明不是同一个文件了，从 0 重读。
//! 3. **检查点要原子写**。tmp + rename，崩溃不会留下半写的偏移。

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use atlas_wist_adapter::envelope_from_sink_line;
use chrono::{SecondsFormat, Utc};
use serde::{Deserialize, Serialize};

const CHECKPOINT_SCHEMA_VERSION: &str = "v1";

/// 单行上限。一帧真实快照约 0.9 MB；给这个上限是为了让「文件被别的格式污染」
/// 变成一条可跳过的坏记录，而不是把内存吃光。
pub const MAX_LINE_BYTES: u64 = 64 * 1024 * 1024;

/// 检查点：读到哪个文件的什么位置。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Checkpoint {
    pub schema_version: String,
    /// 这份检查点属于哪个输入文件。换了文件就要重来，不能沿用偏移。
    pub input: String,
    /// 文件 inode。变了 = 轮转。
    #[serde(default)]
    pub inode: Option<u64>,
    /// 已消费到的字节偏移（下一个待读字节）。
    pub offset: u64,
    pub updated_at: String,
}

impl Checkpoint {
    fn fresh(input: &Path, inode: Option<u64>) -> Self {
        Self {
            schema_version: CHECKPOINT_SCHEMA_VERSION.to_string(),
            input: input.display().to_string(),
            inode,
            offset: 0,
            updated_at: now_rfc3339(),
        }
    }
}

/// 读到的一行，带它在文件里的位置 —— 调用方据此决定「提交到哪」。
///
/// 只要某行处理失败就不该越过它提交，否则下一轮永远不会重试这一行。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LineAt {
    /// 这一行（含换行）的起始偏移。
    pub offset: u64,
    /// 这一行之后的偏移（下一行的起点）。
    pub next_offset: u64,
    /// 行内容，**已去掉结尾换行**。
    pub text: String,
}

/// 增量读取器。
#[derive(Debug)]
pub struct DiscoveryLogTail {
    input: PathBuf,
    checkpoint_path: PathBuf,
    checkpoint: Checkpoint,
    /// 上一轮看到的 inode，用来识别轮转。
    inode: Option<u64>,
}

impl DiscoveryLogTail {
    /// 打开输入与检查点。检查点不存在 / 坏了 / 不属于这个文件 → 从 0 开始。
    pub fn open(
        input: impl Into<PathBuf>,
        checkpoint_path: impl Into<PathBuf>,
    ) -> io::Result<Self> {
        let input = input.into();
        let checkpoint_path = checkpoint_path.into();
        let inode = file_inode(&input)?;

        let checkpoint = match load_checkpoint(&checkpoint_path) {
            Some(checkpoint) if checkpoint.input == input.display().to_string() => checkpoint,
            // 检查点属于别的文件：直接重来。沿用偏移会把新文件的头部当成「已读过」。
            _ => Checkpoint::fresh(&input, inode),
        };

        let mut tail = Self {
            input,
            checkpoint_path,
            checkpoint,
            inode,
        };
        // 打开时就校正一次：进程停机期间文件可能已经轮转。
        tail.rotate_if_needed()?;
        Ok(tail)
    }

    pub fn offset(&self) -> u64 {
        self.checkpoint.offset
    }

    pub fn input(&self) -> &Path {
        &self.input
    }

    /// 读走自上次提交以来新增的**完整行**。不改检查点 —— 提交是调用方的事。
    pub fn poll(&mut self) -> io::Result<Vec<LineAt>> {
        self.rotate_if_needed()?;

        let bytes = fs::read(&self.input)?;
        let start = self.checkpoint.offset as usize;
        if start >= bytes.len() {
            return Ok(Vec::new());
        }

        let mut lines = Vec::new();
        let mut cursor = start;
        while let Some(relative_end) = bytes[cursor..].iter().position(|byte| *byte == b'\n') {
            let line_end = cursor + relative_end;
            let text = String::from_utf8_lossy(&bytes[cursor..line_end]).into_owned();
            let next_offset = (line_end + 1) as u64;
            lines.push(LineAt {
                offset: cursor as u64,
                next_offset,
                text,
            });
            cursor = line_end + 1;
        }

        // 结尾的半行（没有换行）不消费：网关可能正在写。
        Ok(lines)
    }

    /// 把偏移推进到 `offset` 并原子落盘。
    ///
    /// 只允许向前推进；回退说明调用方算错了位置，直接拒绝而不是把已消费的数据再送一遍。
    pub fn commit(&mut self, offset: u64) -> io::Result<()> {
        if offset < self.checkpoint.offset {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "refusing to move checkpoint backwards: {} -> {offset}",
                    self.checkpoint.offset
                ),
            ));
        }
        self.checkpoint.offset = offset;
        self.checkpoint.updated_at = now_rfc3339();
        self.checkpoint.inode = self.inode;
        store_checkpoint(&self.checkpoint_path, &self.checkpoint)
    }

    fn rotate_if_needed(&mut self) -> io::Result<()> {
        let inode = file_inode(&self.input)?;
        let len = fs::metadata(&self.input)?.len();

        let rotated = match (self.inode, inode) {
            (Some(previous), Some(current)) => previous != current,
            // 拿不到 inode（非 unix）时退化成「文件变短 = 轮转」。
            _ => len < self.checkpoint.offset,
        };

        if rotated {
            // 轮转后从 0 重读。重复消费是安全的（`ingest_id` 含 `snap_id`，重放幂等）。
            self.checkpoint.offset = 0;
        } else if len < self.checkpoint.offset {
            // inode 没变但文件短了：被截断（运维手删/清空）。
            self.checkpoint.offset = 0;
        }
        self.inode = inode;
        Ok(())
    }
}

fn now_rfc3339() -> String {
    Utc::now().to_rfc3339_opts(SecondsFormat::Micros, true)
}

#[cfg(unix)]
fn file_inode(path: &Path) -> io::Result<Option<u64>> {
    use std::os::unix::fs::MetadataExt;
    Ok(Some(fs::metadata(path)?.ino()))
}

#[cfg(not(unix))]
fn file_inode(path: &Path) -> io::Result<Option<u64>> {
    fs::metadata(path)?;
    Ok(None)
}

/// 读检查点。坏 JSON / 读不动都当作「没有」—— 从 0 重来的代价只是重放一遍（幂等）。
pub fn load_checkpoint(path: &Path) -> Option<Checkpoint> {
    let raw = fs::read_to_string(path).ok()?;
    let checkpoint: Checkpoint = serde_json::from_str(&raw).ok()?;
    if checkpoint.schema_version != CHECKPOINT_SCHEMA_VERSION {
        return None;
    }
    Some(checkpoint)
}

/// 原子写检查点：同目录 tmp + rename。
pub fn store_checkpoint(path: &Path, checkpoint: &Checkpoint) -> io::Result<()> {
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent)?;
    }
    let tmp = path.with_extension("tmp");
    let body = serde_json::to_vec_pretty(checkpoint)
        .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))?;
    fs::write(&tmp, body)?;
    fs::rename(&tmp, path)
}

pub use atlas_wist_adapter::{AdapterError, EdgeEnvelope};

/// 把一行 sink 记录转成 atlas 的输入信封。
///
/// 这一层只判**永久性**错误（JSON 坏了 / 没有 `body` / 快照缺 host）：这些行重试一万次
/// 也不会变好，调用方应该跳过并推进偏移。写库失败不在这里，那是调用方的事。
pub fn convert_sink_line(
    line: &str,
    tenant: &str,
    env: Option<&str>,
) -> Result<atlas_domain::AtlasInputEnvelope, String> {
    let envelope = envelope_from_sink_line(line, tenant, env).map_err(|err| err.to_string())?;
    // 适配器输出与 `AtlasInputEnvelope` 是同一份 JSON，只是类型不同。
    // 用序列化对一次比手抄字段稳 —— 两边形状一漂就会在这里显形。
    let value = serde_json::to_value(&envelope)
        .map_err(|err| format!("serialize converted envelope: {err}"))?;
    serde_json::from_value(value).map_err(|err| format!("rebuild atlas envelope: {err}"))
}

#[cfg(test)]
mod tests;
