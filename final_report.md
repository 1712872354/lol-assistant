# 项目修复终报告（final_report）

- 日期：2026-09-24
- 范围：`review.md` 的 P0–P3 全量改进 + 遗留债第二轮清理（TDD 红绿重构）
- 流程：批次执行 + 检查点（CP1–CP4 + 清债轮），全程测试锁定行为

---

## 一、验证结果（终态）

| 关卡 | 结果 |
|------|------|
| `cargo test` | **129 passed**（基线 115 → +14 回归；phase_label_all 随字段删除移除） |
| `cargo clippy --all-targets -- -D warnings` | **0 警告** |
| `cargo fmt --all -- --check` | 无 diff |
| `pnpm --dir frontend test`（vitest） | **11 passed**（5 文件；含纯函数测试 7 个） |
| `pnpm --dir frontend typecheck` | 0 错误 |
| `pnpm --dir frontend lint`（ESLint） | 0 错误 / 2 warn（set-state-in-effect，见遗留） |
| `pnpm --dir frontend build` | 通过 |

---

## 二、修复清单

### 批次 1（P0 正确性）
| 项 | 内容 | 回归测试 |
|----|------|---------|
| C1 | `Monitor` 探测移出 `inner` 锁（临界区只做 CPU 操作），消除 53s 级 UI 假死 / 退出挂起 / 潜伏死锁 | `probe_does_not_hold_inner_lock` |
| C2 | `remake` 单点口径 `is_remake()`（summary/detail/SGP 共用） | `remake_consistent_between_summary_and_detail` 等 ×2 |
| C3 | `MatchPage` → `total: Option<i32>` + `total_pages: Option<i32>` + `has_more`，删除伪 game_count | `match_page_sgp_total_is_unknown` 等 ×2 |

### 批次 2（P1 安全与错误处理）
| 项 | 内容 | 回归测试 |
|----|------|---------|
| T2.1 | 去 `ghp.ci` 镜像；删 Info 死字段；反回滚闸 `version_newer` | ×3 |
| T2.2 | CSP 收紧；`Credentials` Debug 掩码 token | ×2 |
| T2.3 | 统一 `AppError`（7 变体）并入 `LcuError`；消灭 `contains("404")` | ×2 |
| T2.4 | gameinfo 错误态可见；三视图独立 ErrorBoundary；无嵌套交互；UpdateDialog dialog 语义 + 焦点陷阱 | vitest ×4 |

### 批次 3（P2 结构重构）
| 项 | 内容 |
|----|------|
| T3.1 | `parse_match_detail` 拆 5 段；`gameinfo.rs` 2194→650（model/protocol/cache/util/tests）；`history.rs` 拆 asset_cache/sgp_tokens |
| T3.2 | `MatchDetailPanel` 475→255、`PlayerSlotCard` 381→228；公共层 `lib/`（AssetImg+useAsset/RatioBar/tone/rank/summoner）；Tone/toSummonerResult 各收敛为一份 |
| T3.3 | `kill_pct`→`kill_participation`；三处 `rating`→matchRating/playerScore/teamScore；`RankedInfo.summoner_id`→`query_id`；`queue_filter [-1]`→`Option`；`PlayerKey` 枚举 |
| T3.4 | `queue_table` 静态化；SGP 闸门 `Semaphore(2)`；`KeyGate` single-flight ×3；`join_all` 上界 `Semaphore(4)`；缓存键去 Debug 格式化 | 测试 ×4 |

### 批次 4（P3 工程化）
| 项 | 内容 |
|----|------|
| T4.1 | `README.md`；`.editorconfig`（utf-8）；CI `--locked`；release 版本注入补 frontend；删 `.md5` 残留 |
| T4.2 | ESLint 基建（flat + typescript-eslint + react-hooks） |
| T4.3 | 删死字段：`self_team_index`/`comp_score`/`MatchSummary.{time,map_name,total_heal,cs,team_id}`/`PlayerRow.{total_heal,cs}` 及对应输入字段；`page_size` 拆 `career_limit` |
| T4.4 | `WailsFn`→`InvokeFn`；删 `fetchRanked`；评分权重常量；`SECS_PER_DAY`；`main.rs` 注释中文化 |

### 清债轮（第二轮：Major 2 项 + Minor 5 项）
| 项 | 内容 | 回归测试 |
|----|------|---------|
| T5.1 | **可空性统一（Major）**：PlayerSlot/RecentMatch/PlayerRow/PlayerRef 标量全 `Option<T>` + 统一 `skip_serializing_if = "Option::is_none"`；删除 `is_zero_*`/`is_false` 哨兵 helper；"0"/空串哨兵在构造点 `opt_id`/`opt_name` 归一（散布 7+ 处的 `== "0"` 判断全部消灭） | 既有 129 锁行为 |
| T5.2 | **phase 文案前端单源（Major）**：删 Rust `phase_label_cn` + `TeamView.phase_label`（连带 UI 冗余字段）；前端 `lib/phase` 为唯一口径，`TeamView` 不再下发文案 | `phase_label_all` 随源删除 |
| T5.3 | **tier 中→英往返清除**：删 `TIER_EN` 表 + `tierToEn`（22 行映射），列表头按设计注释直接展示段位串 | format 测试覆盖 |
| T5.4a | **util 单源**：新建 `src-tauri/src/util.rs`（path_escape/query_escape/flex_value/de_flex_str/opt_id/opt_name）；三处 flex 解析、两处 path_escape 收敛；**SGP URL 转义补全**（原本地实现仅转义 `/`，query 元字符未编码 → 完整 percent-encode，query 注入面关闭） | 既有测试 |
| T5.4b | **日志时间戳**：日志行含 `YYYY-MM-DD HH:MM:SS`（跨午夜可辨） | — |
| T5.4c | **轮询可终止**：`spawn` 保存 `AbortHandle`，`stop()` 真正终止检测循环（m8 注释不实已一并修正为行为） | 既有 monitor 测试 |
| T5.4d | **前端纯函数测试**：fmtK/fmtNum/resultOf/rankedDisplay/splitRank/threatTone/relTime 7 个用例（vitest 5 文件 11 用例） | 新增 ×7 |

---

## 三、有意偏离与简化（相对 `plan.md`）

1. **T3.1 gameinfo 拆分为 5 文件**（model/protocol/cache/util/tests）而非计划 8 文件：同等消灭巨型文件，降低跨文件可见性改造风险。
2. **T2.2 保留 `ConnStatus` 于原锁**：探测移出锁后临界区无 I/O，独立快照收益趋零。
3. **相对条统一为 `RatioBar` + 双色调映射**：`StatChip` 内嵌条保留 span 结构（div-in-span 合法性）仅共享映射表。
4. **可空性统一的原则分层**：对外视图模型全 `Option`；`PlayerRef` 中间态的 `profile_icon_id/champion_id` 保留 `i32`（解析层输入即数字，0=无）——边界统一 Option，内部数值约定仅存于解析层且有注释。
5. **`update.rs` 格式化正则未改 OnceLock**：仅更新检查时调用（低频）。

## 四、遗留债务（建议下轮处理）

| 级别 | 项 |
|------|-----|
| Minor | 2 条 `react-hooks/set-state-in-effect` warn（useAsset/SummonerTabs 缓存→state 同步；正解是 `useSyncExternalStore`，已降级提示） |
| Minor | `queueOptions.ts` 分组清单与 `parser/queue.rs` 队列元数据仍双份维护（M7 残余：建议后端导出 `get_queue_groups` 命令，前端启动加载） |
| Minor | tier 展示串仍由 Rust `tier_cn` 生成中文串下发（M4 残余：建议 `RankedInfo` 改结构化 `{tier_en, division, lp}`，前端持有唯一 EN→CN 映射） |
| Minor | `RankedInfo.solo/flex` 仍为展示字符串（同 M4，与上一条一起改） |
| Minor | `cmdline` 真机冒烟测试 CI 上恒 skip；日志跨午夜不轮转文件（时间戳已有） |

## 五、交付物索引

- `review.md`：审查报告（问题分级与证据）
- `plan.md`：实施计划（任务分解与检查点）
- `final_report.md`：本文件
- 新增模块：`src-tauri/src/{error,util}.rs`、`service/{asset_cache,sgp_tokens,key_gate}.rs`、`service/gameinfo/{model,protocol,cache,util,tests}.rs`、`frontend/src/lib/{tone,rank,summoner,RatioBar,AssetImg}.ts(x)`
- 新增测试：Rust +14、前端 +11（vitest 基建新建，5 测试文件）
- 工程底座：`README.md`、`.editorconfig`、`frontend/eslint.config.js`
