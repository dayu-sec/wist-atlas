//! 增量读取的边界用例：半行、轮转、截断、检查点归属、回退拒绝。

use std::fs;
use std::io::Write;
use std::path::PathBuf;

use super::*;

struct TempDir(PathBuf);

impl TempDir {
    fn new(name: &str) -> Self {
        let suffix = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("atlas-wist-ingest-{name}-{suffix}"));
        fs::create_dir_all(&dir).expect("create temp dir");
        Self(dir)
    }

    fn log(&self) -> PathBuf {
        self.0.join("agent-discovery.jsonl")
    }

    fn checkpoint(&self) -> PathBuf {
        self.0.join("state/offset.json")
    }

    fn append(&self, text: &str) {
        let mut file = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(self.log())
            .expect("open log");
        file.write_all(text.as_bytes()).expect("append");
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).ok();
    }
}

fn texts(lines: &[LineAt]) -> Vec<&str> {
    lines.iter().map(|line| line.text.as_str()).collect()
}

#[test]
fn reads_complete_lines_from_the_start() {
    let dir = TempDir::new("from-start");
    dir.append("a\nb\nc\n");
    let mut tail = DiscoveryLogTail::open(dir.log(), dir.checkpoint()).expect("open");

    let lines = tail.poll().expect("poll");
    assert_eq!(texts(&lines), vec!["a", "b", "c"]);
    assert_eq!(tail.offset(), 0, "poll 不推进检查点");
}

#[test]
fn a_partial_trailing_line_is_not_consumed() {
    // 网关可能正在写一帧（几百 KB）。读到一半的半行绝不能进解析器。
    let dir = TempDir::new("partial");
    dir.append("a\n{\"body\":\"still-writ");
    let mut tail = DiscoveryLogTail::open(dir.log(), dir.checkpoint()).expect("open");

    let lines = tail.poll().expect("poll");
    assert_eq!(texts(&lines), vec!["a"]);
    assert_eq!(lines[0].next_offset, 2);

    // 写完了：下一轮就能看到它。
    dir.append("ing\"}\n");
    let lines = tail.poll().expect("poll again");
    assert_eq!(texts(&lines), vec!["a", "{\"body\":\"still-writing\"}"]);
}

#[test]
fn commit_makes_the_next_poll_start_after_it() {
    let dir = TempDir::new("commit");
    dir.append("a\nb\n");
    let mut tail = DiscoveryLogTail::open(dir.log(), dir.checkpoint()).expect("open");

    let lines = tail.poll().expect("poll");
    tail.commit(lines[0].next_offset).expect("commit");
    assert_eq!(tail.offset(), 2);

    let lines = tail.poll().expect("poll");
    assert_eq!(texts(&lines), vec!["b"]);

    tail.commit(4).expect("commit all");
    assert!(tail.poll().expect("poll").is_empty());
}

#[test]
fn the_checkpoint_survives_a_reopen() {
    let dir = TempDir::new("reopen");
    dir.append("a\nb\n");
    {
        let mut tail = DiscoveryLogTail::open(dir.log(), dir.checkpoint()).expect("open");
        tail.commit(2).expect("commit");
    }

    let mut tail = DiscoveryLogTail::open(dir.log(), dir.checkpoint()).expect("reopen");
    assert_eq!(tail.offset(), 2);
    assert_eq!(texts(&tail.poll().expect("poll")), vec!["b"]);
}

#[test]
fn a_rotated_file_is_read_from_the_start() {
    // 轮转 = 新文件（inode 变了）。沿用旧偏移会把新文件的头部当成「已读过」。
    let dir = TempDir::new("rotate");
    dir.append("old-1\nold-2\n");
    let mut tail = DiscoveryLogTail::open(dir.log(), dir.checkpoint()).expect("open");
    tail.commit(12).expect("commit both");

    fs::remove_file(dir.log()).expect("rotate away");
    dir.append("new-1\n");

    let lines = tail.poll().expect("poll after rotation");
    assert_eq!(
        texts(&lines),
        vec!["new-1"],
        "轮转后必须从 0 重读，而不是跳过"
    );
}

#[test]
fn a_truncated_file_is_read_from_the_start() {
    // inode 没变但文件短了：被清空 / 手删。旧偏移已经越界。
    let dir = TempDir::new("truncate");
    dir.append("a\nb\nc\n");
    let mut tail = DiscoveryLogTail::open(dir.log(), dir.checkpoint()).expect("open");
    tail.commit(6).expect("commit");

    let mut file = fs::OpenOptions::new()
        .write(true)
        .truncate(true)
        .open(dir.log())
        .expect("truncate");
    file.write_all(b"x\n").expect("write");

    let lines = tail.poll().expect("poll after truncate");
    assert_eq!(texts(&lines), vec!["x"]);
}

#[test]
fn a_checkpoint_from_another_file_is_ignored() {
    let dir = TempDir::new("foreign-checkpoint");
    dir.append("a\nb\n");
    let other = Checkpoint {
        schema_version: CHECKPOINT_SCHEMA_VERSION.to_string(),
        input: "/somewhere/else.jsonl".to_string(),
        inode: None,
        offset: 4,
        updated_at: "2026-10-25T00:00:00Z".to_string(),
    };
    store_checkpoint(&dir.checkpoint(), &other).expect("store");

    let tail = DiscoveryLogTail::open(dir.log(), dir.checkpoint()).expect("open");
    assert_eq!(tail.offset(), 0, "检查点属于别的文件 → 从头来");
}

#[test]
fn a_corrupt_checkpoint_falls_back_to_the_start() {
    // 宁可重放一遍（幂等），也不要因为一个坏文件永远停在原地。
    let dir = TempDir::new("corrupt-checkpoint");
    dir.append("a\n");
    fs::create_dir_all(dir.checkpoint().parent().expect("parent")).expect("mkdir");
    fs::write(dir.checkpoint(), b"{ not json").expect("write");

    let tail = DiscoveryLogTail::open(dir.log(), dir.checkpoint()).expect("open");
    assert_eq!(tail.offset(), 0);
}

#[test]
fn commit_refuses_to_move_backwards() {
    let dir = TempDir::new("backwards");
    dir.append("abcdef\n");
    let mut tail = DiscoveryLogTail::open(dir.log(), dir.checkpoint()).expect("open");
    tail.commit(4).expect("commit");

    let err = tail.commit(2).expect_err("must refuse");
    assert_eq!(err.kind(), io::ErrorKind::InvalidInput);
    assert_eq!(tail.offset(), 4, "拒绝之后偏移不动");
}

#[test]
fn an_empty_file_yields_nothing() {
    let dir = TempDir::new("empty");
    dir.append("");
    let mut tail = DiscoveryLogTail::open(dir.log(), dir.checkpoint()).expect("open");
    assert!(tail.poll().expect("poll").is_empty());
}

#[test]
fn a_missing_input_file_is_an_error_not_a_silent_empty_batch() {
    // 「文件不存在」和「没有新数据」必须能区分：前者通常意味着路径配错了，
    // 静默当空会让 worker 看起来正常但永远不工作。
    let dir = TempDir::new("missing");
    let err = DiscoveryLogTail::open(dir.log(), dir.checkpoint()).expect_err("must fail");
    assert_eq!(err.kind(), io::ErrorKind::NotFound);
}

#[test]
fn offsets_account_for_every_byte() {
    let dir = TempDir::new("offsets");
    dir.append("ab\n\ncde\n");
    let mut tail = DiscoveryLogTail::open(dir.log(), dir.checkpoint()).expect("open");

    let lines = tail.poll().expect("poll");
    assert_eq!(texts(&lines), vec!["ab", "", "cde"]);
    assert_eq!(lines[0].offset, 0);
    assert_eq!(lines[0].next_offset, 3);
    assert_eq!(lines[1].offset, 3);
    assert_eq!(lines[1].next_offset, 4);
    assert_eq!(lines[2].offset, 4);
    assert_eq!(lines[2].next_offset, 8);
}
