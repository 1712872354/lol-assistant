# lol-assistant 修复实施计划（P0–P3）

- 依据：`review.md`（2026-09-24 全项目审查）
- 测试策略：TDD 红绿重构——每个修复先写失败测试（Red），再最小实现（Green），最后重构
- 批次：4 批，每批结束跑全量测试 + 人工检查点
- 完成定义（DoD）：`cargo test` / `cargo clippy --all-targets -- -D warnings` / `cargo fmt --check` / `pnpm typecheck` / `pnpm build` 全绿

---

## 批次 1：P0 正确性缺陷（3 项）

### T1.1 修复 Monitor 持锁跨探测（C1）
- 文件：`src-tauri/src/lcu/monitor.rs`、`src-tauri/src/lib.rs`
- Red：新增测试 `probe_does_not_hold_inner_lock`——probe 内 sleep 500ms 期间调用 `status()`，断言 100ms 内返回（当前会超时失败）
- Green：`tick()` 改为「短锁取 creds → 释放锁 → probe → 短锁提交」；`ConnStatus` 拆独立 `RwLock` 快照供 `status()` 无阻塞读
- 重构：`lib.rs:177` 退出路径去掉 `block_on(mon.stop())` 对 probe 锁的依赖（改为中止任务后放行）
- 验证：新增测试通过 + 既有 monitor 状态机测试不回归

### T1.2 `remake` 派生口径单点化（C2）
- 文件：`src-tauri/src/parser/lcu_match.rs`、`src-tauri/src/parser/sgp_match.rs`
- Red：新增测试 `remake_consistent_between_summary_and_detail`——构造仅 `team_early_surrendered=true` 的对局，断言 `build_summary().remake == parse_match_detail().remake == true`（当前 summary 为 false，失败）
- Green：提取 `fn is_remake(game_ended_in_early_surrender: bool, team_early_surrendered: bool) -> bool`（或等价单点函数），summary/detail/SGP 三处共用
- 重构：删除 SGP 适配层 `sgp_match.rs:204` 的折算补偿逻辑（不再需要「碰巧判对」）
- 验证：新测试 + `sgp_match.rs:296` 既有测试口径一致

### T1.3 `MatchPage` 分页语义修正（C3）
- 文件：`src-tauri/src/service/history.rs`、`src-tauri/src/commands.rs`、`frontend/src/lib/types.ts`、`frontend/src/features/history/MatchList.tsx`
- Red：新增测试 `match_page_sgp_total_is_unknown`——SGP 路径断言 `total == None` 且 `has_more` 按启发式给；LCU 路径断言 `total == Some(真实总数)`（当前字段名/语义不符，失败）
- Green：`MatchPage` 改 `total: Option<i32>` + `has_more: bool`；SGP 传 `None`，LCU 传 `Some(game_count)`；删除 `total_pages` 启发式与 `page_size` 无消费字段（若前端确认无用）
- 重构：前端 `types.ts` 同步契约；`MatchList.tsx` 用 `has_more` 控制翻页禁用（沿用既有逻辑），删除 `void gameCount`
- 验证：Rust 测试 + `pnpm typecheck`

---

## 批次 2：P1 安全与错误处理

### T2.1 更新链信任面收紧
- 文件：`src-tauri/tauri.conf.json`、`src-tauri/src/update.rs`、`src-tauri/src/update.rs`（Info 结构）
- Red：新增测试 `info_has_no_dead_sha_field` / 配置断言 `endpoints 不含 ghp.ci`（配置测试可读文件断言）
- Green：移除 `ghp.ci` 镜像端点；删除 `Info.sha256/setup_url/portable_url` 死字段（同步 `frontend/src/stores/updateStore.ts:44`）；`download_and_install` 加版本单调性校验（拒绝低于 current）
- 验证：`cargo test` + 配置断言

### T2.2 CSP 收紧 + Credentials Debug 掩码
- 文件：`src-tauri/tauri.conf.json`、`src-tauri/src/lcu/types.rs`
- Red：新增测试 `credentials_debug_masks_token`——格式化 `Credentials` 断言 token 不出现在输出（当前失败）
- Green：`Credentials` 手写 `Debug` 掩码 token 字段；`tauri.conf.json` CSP 设 `default-src 'self'; img-src 'self' asset: data:; style-src 'self' 'unsafe-inline'`
- 验证：新测试 + 前端 `pnpm build` 仍通过（CSP 不误伤）

### T2.3 统一错误类型 `AppError`
- 文件：新建 `src-tauri/src/error.rs`；改动 `commands.rs`、`service/*.rs`、`update.rs`、`portable_updater.rs`、`sgp/mod.rs`、`liveclient/mod.rs`、`lcu/*.rs`
- Red：新增测试 `app_error_variants_are_matchable`——构造 `AppError::NotFound` 断言可按变体匹配（当前只能匹配 String，失败）
- Green：定义 `AppError`（thiserror）分 `NotConnected / Http / Parse / NotFound / Updater / Internal` 变体；内部签名改 `Result<T, AppError>`；`commands.rs` 边界 `impl From<AppError> for String`（用户文案映射）；消灭 `monitor.rs:316` 的 `contains("404")`
- 重构：`lcu/client.rs::LcuError` 并入 `AppError`；错误文案（用户向）与日志文案（开发向）分离
- 验证：全量测试 + clippy 零警告

### T2.4 前端止血
- 文件：`frontend/src/stores/gameinfoStore.ts`、`frontend/src/App.tsx`、`frontend/src/components/UpdateDialog.tsx`、`frontend/src/features/gameinfo/PlayerSlotCard.tsx`、`frontend/src/features/history/SummonerTabs.tsx`、`frontend/src/components/ErrorBoundary.tsx`
- Red：新增测试（若引入 vitest）或手工核对清单——本项以结构断言为主：ErrorBoundary 包裹每个视图、UpdateDialog 有 `role="dialog"`、无 button 嵌套交互
- Green：
  - `gameinfoStore.ts:119` 静默 catch → 记日志 + 写 `error` 状态字段
  - `App.tsx` 给 KeepAlive 三页各包一层 `ErrorBoundary`
  - `PlayerSlotCard.tsx:205`、`SummonerTabs.tsx:107` 嵌套交互元素改为兄弟节点/`div role="button"`；关闭按钮去掉 `tabIndex={-1}`
  - `UpdateDialog.tsx` 加 `role="dialog" aria-modal="true"` + 焦点陷阱 + 焦点返还
- 验证：`pnpm typecheck` + `pnpm build`

---

## 批次 3：P2 结构重构

### T3.1 拆分 Rust 巨型单元
- `parser/lcu_match.rs`：`parse_match_detail`（223 行）拆 `build_rows / normalize_damage / group_teams / summarize_teams`
- `service/gameinfo.rs`（2183 行）拆 `service/gameinfo/{mod,model,lobby,champ_select,live,career,cache,util}.rs`
- `service/history.rs`（1645 行）拆出 `asset_cache.rs`、`sgp_tokens.rs`
- 测试策略：既有测试全绿即视为重构正确（纯移动 + 函数拆分不改行为）；为 `parse_match_detail` 各子步骤补单元测试锁行为
- 验证：`cargo test` 全绿、clippy 零警告

### T3.2 前端拆分 + 公共层上移
- `MatchDetailPanel.tsx`（475 行）拆 `PlayerRow / ColumnHeader / StatBar / badges.ts`
- `PlayerSlotCard.tsx`（384 行）拆 `SlotSkeleton / PlayerCardHeader / RecentMatchList`
- `AssetImg`、相对条（StatBar/ThreatBar/StatChip 统一为 `RatioBar`）、`Tone`、`toSummonerResult`、`UNRANKED` 常量上移 `frontend/src/lib/`
- 验证：`pnpm typecheck` + `pnpm build`；`Tone` 只剩一份定义

### T3.3 统一可空性与派生口径
- 视图模型统一 `Option<T>`：`PlayerSlot.puuid/profile_icon_id`、`PlayerRow.puuid/summoner_id` 等；删除 `is_empty/is_zero/is_false` 混用 skip 策略，统一 `skip_serializing_if = "Option::is_none"`
- 魔法值改 Option/枚举：`queue_filter: [-1]` → `enum QueueFilter { Follow, All, Ids(Vec<i32>) }`；`summoner_id == "0"` → `Option<String>`；「未定级」→ `Option<RankedTier>`
- `PlayerKey` 枚举替代 `is_puuid` 启发式（`history.rs:845`）
- 队列/phase/tier 文案单点化：后端 `parser/queue.rs` 导出常量 + 新命令 `get_queue_table`（或构建期生成 `frontend/src/lib/queueTable.ts`）；删除 `queueOptions.ts` 手工清单与 `TIER_EN` 反向映射（DTO 改传结构化 tier/division/lp）
- 命名修正：`RankedInfo.summoner_id` → `query_id`；`kill_pct` → `kill_participation`（或换算口径 ≤100）；三处 `rating` 按尺度改名（`match_rating / team_score / player_score`）
- 验证：全量测试 + typecheck + build；前端 `?? 0` 兜底数量显著下降

### T3.4 并发放量治理
- `service/gameinfo.rs` 的 `join_all` 加 `Semaphore` 上界（建议 4）
- `sgp/mod.rs` 加并发闸门（建议 `Semaphore(2)`，对齐 LCU）
- 缓存 single-flight：`fetch_career`、`fetch_sgp_tokens`、`get_champ_index`、`lookup_index` 用 `Mutex<HashMap<K, Shared<Box<Future>>>>` 或 `tokio::sync::OnceCell` 变体
- `parser/queue.rs:24` `queue_table()` 改 `LazyLock<HashMap>` 或 `match` 跳转表返回 `&'static QueueInfo`
- `gameinfo.rs:974` 缓存键改结构化 key（`(String, Vec<i32>)` 元组），弃用 `format!("{filter:?}")`
- Red：新增测试 `queue_table_is_not_rebuilt_per_call`（可用计数或直接断言返回 `&'static`）；`sgp_concurrency_is_bounded`（FakeHttp 记录并发峰值断言 ≤ 上限）
- 验证：全量测试

---

## 批次 4：P3 工程化与规范化

### T4.1 文档与配置底座
- 新建 `README.md`（项目简介、构建、发版、Windows-only 声明、目录结构）
- 新建 `.editorconfig`（`charset = utf-8`、indent 规则对齐 rustfmt/tsconfig）
- `package.json` 根补 `version: 1.0.7`；`release.yml` 「Inject version」补写 `frontend/package.json`
- 删除 `frontend/package.json.md5` 残留
- CI：`cargo test` 加 `--locked`；补 `cargo audit`（或 `cargo deny`）步骤
- 验证：CI 干跑（act 不可用则人工核对 YAML）

### T4.2 前端 Lint 与最小测试
- 引入 ESLint（`@typescript-eslint` + `eslint-plugin-react-hooks`）+ 配置；`pnpm lint` 脚本；修掉现有告警（含 `MatchList.tsx:84` 死 eslint-disable）
- 引入 vitest + 最小测试：`format.ts` 纯函数、`RatioBar`、`toSummonerResult`
- 验证：`pnpm lint` / `pnpm test` 全绿

### T4.3 死字段与语义清理
- 删 `self_team_index`（恒 0）、`comp_score`（恒等于 win_rate）、`MatchSummary.{time,mapName,totalHeal,cs,teamId}` 等未消费字段（前端确认无消费后删）
- `page_size` 拆 `page_size` + `career_limit` 两配置项（`config.rs`、`commands.rs:32-34`、`SettingsView.tsx:70` 文案）
- `TeamView.phase_label` 上提至 `ViewState` 单份
- 验证：全量测试 + typecheck + build

### T4.4 规范化收尾
- 补服务层中文 `///` 文档（`history.rs`/`gameinfo.rs`/`parser/*` 的 pub API，目标覆盖 80%+）
- 提取重复工具到 `src-tauri/src/util.rs`：`flex_str/de_flex_str`、`path_escape`、`(5..=50)` 页大小校验、更新进度百分比
- 魔法数字具名化：`rating()` 权重（`lcu_match.rs:531`）、`86400`（`logging.rs:52`）、前端防抖/阈值常量
- 英文注释转中文（`main.rs:1` 保留 Tauri 模板原文可豁免并注明）
- `update.rs:60-103` 正则改 `OnceLock` 集中初始化
- 删除 `fetchRanked` 死别名、`WailsFn` 改 `InvokeFn`
- 验证：全量测试 + lint + clippy

---

## 人工检查点

| 检查点 | 时机 | 内容 |
|--------|------|------|
| CP1 | 批次 1 完成 | 复核并发修复不引入新死锁；review.md 的 C1/C2/C3 是否可关闭 |
| CP2 | 批次 2 完成 | 复核 CSP 不误伤前端资源加载；AppError 文案对用户可读 |
| CP3 | 批次 3 完成 | 复核重构后行为等价（重点：对局页/战绩页手测） |
| CP4 | 批次 4 完成 | 终验 + 生成 `final_report.md` |

## 偏离处理

- 子代理偏离计划：立即停止、分析偏差、调整计划或重新派发（Superpowers 规程）
- 测试失败：回到 Red 阶段分析根因，修测试或修代码，不跳过
- 发现新问题：记录到 `review.md` 增补节，不擅自扩大范围
