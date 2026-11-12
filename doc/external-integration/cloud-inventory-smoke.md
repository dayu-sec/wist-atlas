# 云资产接入 smoke：真账号 → 真库

单元用例和 `--ignored` 真库用例都只用构造数据。**构造数据不会暴露「字段名对不上」「
sync/async 只改了一边」「某条 SQL 从没被执行过」这三类问题** —— 它们恰恰是真账号跑一次
才会露头的。这份手册是那条链路的固定跑法。

## 为什么值得每次改动后跑

第一次跑这套流程时（2026-11-13），一次就抓到两个单测抓不到的 bug：

| 症状 | 真因 |
| --- | --- |
| 库里 20 个子网 / 10 个 VPC 的 `cloud_account` 全是 NULL，退役永远打不到它们 | `atlas-wist-ingest` 只给主机和 EIP 写了 `cloud_account` |
| 子网的 `cloud_account` 补上后仍然是 NULL，`ingest_scope` 也是 NULL | `materialize_candidates_async` **没跟着 sync 一起改** —— 而生产路径走的正是 async |
| 某个 VPC 的子网全删光那一轮，整条 ingest 报 `error serializing parameter 0` | `get_network_domain` 的 SQL 写成 `= $1`，参数是 `&str`，tokio-postgres 按列类型推成 `uuid` |

## 链路

```
wist-harvest --source <云账号源>  ──TCP 帧──▶  capture.py 落盘
                                                │
                              make_rounds.py 造多轮（每轮少一台/少一个子网）
                                                │
                          atlas-wist-ingest --store postgres  ──▶  PG
                                                                  │
                                                    psql 断言（下表）
```

## 步骤

```sh
# 1. 真采集（凭据走 sec-value.yml，不要写进命令行）
python3 capture.py out/frame.bin 19501 &
wist-harvest once --config harvest.toml --source sources.d/tencent-cloud-prod.toml --fresh

# 2. 造轮次：round1 = 原始快照，round2..N = 每轮少一台主机
python3 make_rounds.py out/frame.bin out/rounds 4

# 3. 灌库（每轮一个独立 checkpoint，否则第二轮会被当成已消费）
for r in 1 2 3 4; do
  DATABASE_URL=... atlas-wist-ingest --input out/rounds/round$r.jsonl \
    --checkpoint out/rounds/ck$r --tenant <TENANT_UUID> --store postgres --once
done
```

## 断言清单

跑完 round1 就该成立（**注意 `no_acct` 三列必须全是 0**，否则退役逻辑对该类对象永久失效）：

| 检查 | 期望 |
| --- | --- |
| `host_inventory` 行数 | ≥ 快照里的 `CLOUD_HOST` 条数 |
| `host_inventory.cloud_instance_id` 非空 | 全部（云侧来源） |
| `network_segment` / `network_domain` / `elastic_ip` 的 `cloud_account is null` | **0** |
| `network_segment.ingest_scope is null` | **0** |
| `host_net_assoc where valid_to is null` | 有私网 IP 的主机应各有一条 |

```sql
select (select count(*) from host_inventory) hosts,
       (select count(*) from network_segment where cloud_account is null) seg_noacct,
       (select count(*) from network_domain where cloud_account is null) dom_noacct,
       (select count(*) from elastic_ip where cloud_account is null) eip_noacct,
       (select count(*) from network_segment where ingest_scope is null) seg_noscope;
```

跑完第 3 个「缺席」轮之后：

| 对象 | 期望 |
| --- | --- |
| 被移除的主机 | `stale_at` 非空，`miss_count = 3` |
| 被移除的子网 | 同上；**行不删除** |
| 被移除的 EIP | 同上 |
| 子网还在的 VPC | `stale_at` **为空**（不能因为同账号别的段没了就退役） |
| 子网全没了的 VPC | `stale_at` 非空 |

再把原始快照灌一轮（新 `snap_id`），所有 `stale_at` 必须被清空、`miss_count` 归零 ——
**退役必须可逆**，否则一次采集抖动就留下永久错误的目录。

## 不要拿它验的东西

- **用户可见性**：本手册只查库。页面/接口是否能看见这些资产是另一件事（见
  `doc/roadmap/todo-backlog.md` 里 EIP 读接口那条）。
- **性能**：真账号 26 台机的量级说明不了大规模问题。
