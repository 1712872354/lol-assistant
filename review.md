# lol-assistant 全项目代码审查报告

- 审查日期：2026-09-24
- 审查范围：`src-tauri/src`（27 个 Rust 文件，约 8600 行）、`frontend/src`（约 60 个 TS/TSX 文件）、工程配置（Cargo.toml / package.json / CI）
- 审查维度：编码规范、数据结构、架构、并发、安全、工程化
- 方法：静态审查 + `cargo check` 验证（0 warning）+ 子代理并行深挖

---

## 一、总体结论

**综合评分：6.5 / 10**（Rust 6.5 ｜ 前端 7.5 ｜ 数据结构 6.5 ｜ 架构/并发/安全/工程化 6.5）

| 维度 | 评价 |
|------|------|
| 命名规范 | 优：Rust/TS 命名高度合规，仅 Win32 FFI 合理例外 |
| 注释 | 良：绝大多数中文且解释「为什么」；个别英文残留 |
| 错误处理 | 弱：thiserror 孤岛 + 全局 `Result<_, String>` 双轨制 |
| 函数/文件规模 | 弱：5 个超长函数（最长 223 行）、2 个巨型服务文件（2183/1645 行）、2 个巨型组件（475/384 行） |
| 数据结构 | 中：契约字段对应度高，但可空性双轨、派生口径多头、死字段偏多 |
| 并发安全 | 弱：LCU 闸门实现干净，但 Monitor 持锁跨 I/O 是硬伤 |
| 安全 | 中上：更新器签名/防穿越/回滚做得好；CSP null、第三方镜像是债 |
| 工程化 | 中：CI 面全（fmt/clippy/test/typecheck/build）是加分；缺 README、前端 lint/测试 |
| 测试 | Rust 侧较充分（90+ 用例，含 zip 穿越/回滚/状态机专项）；前端 0 测试 |

**一句话**：测试文化与更新器加固明显高于同类个人项目均值，但存在一个会拖死服务层的并发缺陷，以及解析/服务层函数拆分与错误类型统一两笔结构性欠账。

---

## 二、Critical（阻塞，建议立即修复）

### C1. `Monitor` 持锁跨网络探测，最长约 53s 拖死整个 LCU 服务面
- 位置：`src-tauri/src/lcu/monitor.rs:160`（取锁）→ `monitor.rs:192`、`monitor.rs:214`（持锁 `await (self.probe)`）
- 机制：`probe_and_build` 最多 5 次探测 × 600ms 间隔，每次 reqwest 超时 10s（`lcu/client.rs:42`），失败路径持锁 ≈ 52.4s
- 后果：
  1. `MonitorHttp::client()` 每次 HTTP 都要拿同一把锁（`service/http.rs:31-36`）→ 探测期间战绩/对局/连接状态全部命令挂起，UI 假死；
  2. 退出时 `lib.rs:177` `block_on(mon.stop())` 同锁 → 进程退出可被阻塞数十秒；
  3. 潜伏式死锁：未来任何在 probe/回调里调 `monitor.client()/status()` 的改动都会自锁。
- 修复：探测移出锁外（clone creds 后释放锁，探测完短锁提交）；`ConnStatus` 拆独立 `RwLock` 快照供无阻塞读。

### C2. `remake` 同名字段三套派生口径，列表与详情互相矛盾
- `parser/lcu_match.rs:640`（`build_summary`）：漏掉 `team_early_surrendered`；
- `parser/lcu_match.rs:318`（`parse_match_detail`）：含 `team_early_surrendered`；
- `parser/sgp_match.rs:204`：把该字段折进 `game_ended_in_early_surrender` 才「碰巧」判对。
- 后果：同一 LCU 对局若仅 `teamEarlySurrendered=true`，列表卡片显示胜负、详情页显示「无效」。
- 修复：收敛为单点函数 `fn is_remake(g, s) -> bool`，summary/detail/SGP 三处共用。

### C3. `MatchPage.game_count` 字段名与运行时语义分裂（契约破坏）
- `service/history.rs:460`（SGP）：`game_count = beg + summaries.len()` 实为「已扫描末尾下标」；
- `service/history.rs:390`（LCU）：真实总场数；`history.rs:452` 的 `total_pages` 启发式与 LCU 公式语义不同。
- 后果已在前端显形：`MatchList.tsx:78` `void gameCount`（被迫弃用），但 `types.ts:153` 仍声明 `gameCount: number`。
- 修复：拆为 `total: Option<i32>`（未知为 None）+ `has_more: bool`。

---

## 三、Major（重要，按主题归并）

### 1. 错误处理双轨制
- 仅 `lcu/client.rs:17-25` 用 thiserror 定义 `LcuError`，其余 90+ 处签名是 `Result<_, String>`（`commands.rs:22`、`history.rs:361`、`portable_updater.rs:111` 等）。
- 用户文案与日志文案混在同一 String 通道；`monitor.rs:316` 靠 `contains("404")` 判语义；内部细节（路径/状态码）可透传前端。
- `Cargo.toml` 声明 `thiserror = "2"` 基本未用。
- 修复：建全局 `AppError`（thiserror）分 `NotConnected/Http/Parse/Updater` 变体，仅 command 边界转用户文案。

### 2. 超长函数与巨型文件
超长函数（>80 行）：

| 函数 | 行数 | 位置 |
|------|------|------|
| `parse_match_detail` | 223 | `parser/lcu_match.rs:305-527` |
| `run` | 148 | `lib.rs:34-181` |
| `get_players_ranked` | 104 | `service/history.rs:488-591` |
| `build_slots` | 103 | `service/gameinfo.rs:824-926` |
| `roster` | 103 | `service/gameinfo.rs:314-416` |

嵌套深度达 8–10 层（`lcu/monitor.rs:199` depth=10、`lcu/ws.rs:216` depth=9）。
巨型文件：`service/gameinfo.rs` 2183 行（75KB）、`service/history.rs` 1645 行（60KB）；前端 `MatchDetailPanel.tsx` 475 行、`PlayerSlotCard.tsx` 384 行。

### 3. 并发放量无上界 + 缓存无 single-flight
- `service/gameinfo.rs:839-846`、`:877-889`：`join_all` 对 10 名玩家无并发上限；每人最多再扫 4 页 → 峰值 ~40+ 请求。LCU 侧被 `Semaphore(2)` 串行化（排队放大延迟），SGP 侧完全无闸门（`sgp/mod.rs:109-131`，有限流/风控风险）。
- `fetch_career`（`gameinfo.rs:973`）、`fetch_sgp_tokens`（`history.rs:667`）、`get_champ_index`（`gameinfo.rs:1062`）均无并发合并，同 key 并发 miss 重复回源。
- 正向确认：LCU `Semaphore(2)` 闸门本身实现干净（RAII permit、无跨 await 持锁、无重入），无死锁风险。

### 4. 安全债
- **更新链信任面**：`tauri.conf.json:52-55` 端点含未签名第三方代理 `ghp.ci`；工件有 minisign 兜底（防伪造执行），但 `latest.json` 元数据无签名/哈希绑定 → 可投毒版本元数据、阻断更新、降级诱导。`update.rs:188` `Info.sha256` 恒空是误导性死字段。
- **CSP 为 `null`**（`tauri.conf.json:29-31`）：前端渲染对手数据（召唤师名），目前靠 React 默认转义兜底；一旦出现 `dangerouslySetInnerHTML` 类改动即成 WebView XSS → 可 invoke 全部 18 个 command（含 `download_and_install_update`）。`withGlobalTauri: false` 是正确缓解。
- `Credentials` 派生 `Debug`（`lcu/types.rs:43-49`），token 可随 `{:?}` 入日志。已逐点核对 25 处 `log!` 调用，**当前未实际泄漏**，属隐患。
- SGP URL 拼接仅转义 `/`（`sgp/mod.rs:172-175`），`?`/`#`/`%` 未编码，恶意参数可做 query 注入（host 白名单固定 `*.lol.qq.com`，**无 SSRF**）。
- 正向确认：zip 解压防穿越（`enclosed_name` + `Component::Normal` + 白名单 + 拒符号链接 + 流式限额，`portable_updater.rs:111-184`）+ 失败回滚 + 专项测试；LCU 路径白名单 `PATH_PREFIX_ALLOWLIST` + `path_clean`（`lockfile.rs:167-201`）实现质量好；LCU 自签证书跳验证（`client.rs:41`、`ws.rs:110-161`）对 127.0.0.1 写死 base URL 属行业惯例，风险可接受。

### 5. 数据结构：可空性双轨 + 派生口径多头
- **Option 与哨兵值混用**：`ConnStatus` 用 `Option`（`lcu/types.rs:31`）；`PlayerSlot.puuid: String` 非 Option 却 `skip_serializing_if(is_empty)`（`gameinfo.rs:106`）；`summoner_id == "0"` 哨兵散布 7+ 处（`gameinfo.rs:326,521,742,937,1242,1263`）。前端被迫 11 个字段全声明 optional + 到处 `?? 0` 兜底。
- **ID 类型过载**：`commands.rs:79` `summoner_ids: Vec<String>` 混收 summonerId 与 puuid；`history.rs:845` 用 `len>=32 && contains('-')` 启发式判别（脆弱）；`RankedInfo.summoner_id = input_id`（`history.rs:582`）名不副实，前端只能多键回退查找补锅（`format.ts:99`）。
- **派生展示字符串埋入 DTO**：`kda: String`（"Perfect"）、`duration_min: "15分"`、队列中文名、`tier_cn` 全在 Rust 硬编码；前端还要中→英反向映射（`format.ts:19` 的 22 行 `TIER_EN`）——段位走了「英文→中文→英文」往返，说明 DTO 应传结构化数据。
- **领域知识无单一事实源**：队列表 3 处平行（`parser/queue.rs:27` / `queueOptions.ts:13` / `MatchList.tsx:74`）；`GameflowPhase` 文案表 2 处（`gameinfo.rs:171` / `phase.ts:7`）且前端 3 处自行重算，后端 `phase_label` 形同虚设；`Tone` 类型前端 3 份；flex 字符串解析 3 份（`lcu_match.rs:119` / `gameinfo.rs:419` / `history.rs:918`）；`SummonerRaw` 2 份；身份字段组 5 处平行。
- **魔法数字协议**：`queue_filter: [-1]` 表示「跟随当前」（`queueOptions.ts:30` ↔ `gameinfo.rs:1219`）。
- **死/冗余字段穿越 IPC**：`self_team_index` 恒 0（`lcu_match.rs:524`）；`comp_score === win_rate`（`gameinfo.rs:1212`）；`setup_url/portable_url/sha256` 恒空（`update.rs:186`）；`MatchSummary` 的 `time/mapName/totalHeal/cs/teamId` 前端无消费点；`MatchPage.page_size` 无消费点。
- **语义过载**：`page_size` 一字段两用（分页大小 = 对局页近况场数，`commands.rs:32-34`）；`rating` 一词三种尺度（`lcu_match.rs:77` / `gameinfo.rs:157` / `gameinfo.rs:129`）；`killPct` 可 >100，命名误导（`types.ts:188`）。
- **性能失误**：`parser/queue.rs:24` `queue_table()` 每次查询重建 28 条 HashMap，应改 `LazyLock` 或 `match` 跳转表；`gameinfo.rs:974` 缓存键用 `format!("{filter:?}")` 依赖 Debug 输出格式。
- 正向确认：输出 DTO 统一 `rename_all = "camelCase"`；原始 JSON 与视图模型分层清晰；SGP→LCU 形态适配复用 `build_summary` 单点派生；`State`/`TeamId` 用枚举 + 容错反序列化。

### 6. 前端架构债
- **数据获取三轨并行**：history 走 TanStack Query（`MatchList.tsx:41`）、gameinfo 走 Zustand 自管（`gameinfoStore.ts:97`）、SummonerTabs 走本地 useEffect（`SummonerTabs.tsx:41-57`，与 Query 重复拉同一页、缓存互不共享）。
- **仅根级 1 个 ErrorBoundary**（`App.tsx:55`）：KeepAlive 三页常驻 DOM，任一视图渲染崩溃全崩。
- **gameinfo 刷新失败静默吞错**（`gameinfoStore.ts:119` `catch {}` + 吞异常的 `callApp`），用户只见旧数据无错误态。
- **IPC 边界无校验 `as` 断言体系**（`App.tsx:38`、`gameinfoStore.ts:47,170,174` 等 5 处），字段缺失静默产脏 UI；`normalizeView` 的 `slice(0,2)` 会静默丢弃竞技场第 3 支队伍（`gameinfoStore.ts:52` vs `types.ts:204`）。
- `MatchList.tsx:84-99` 用 `eslint-disable` 压依赖数组——而项目根本没装 ESLint，该指令是死指令。
- DRY：相对条组件 3 份（`MatchDetailPanel.tsx:40` / `PlayerSlotCard.tsx:48` / `TeamPanel.tsx:28`）；`SummonerResult` 构造整段重复（`MatchDetailPanel.tsx:343` vs `PlayerSlotCard.tsx:157`）；gameinfo 跨包 import history 内部的 `AssetImg`（`PlayerSlotCard.tsx:2`）。
- a11y 硬伤：`<button>` 内嵌 `role="button"` 的 `<span>`（`PlayerSlotCard.tsx:205-219`、`SummonerTabs.tsx:107-118`，后者关闭按钮还 `tabIndex={-1}`）；自绘模态框无 `role="dialog"`/焦点陷阱（`UpdateDialog.tsx:33-34`）。
- 正向确认：全库零 `any`、tsconfig 全 strict、非空断言仅 1 处；竞态防护（请求序号/alive 标志）做得细；注释质量高。

### 7. 工程化欠账
- **无 README**（根目录无 `README.md`，仅 `CHANGELOG.md`、`docs/RELEASE.md`）。
- **前端无 ESLint/Prettier、0 测试**；仅 `tsc --noEmit`。
- Rust 测试较充分（13 个文件含 `#[cfg(test)]`、约 90+ 用例），但无独立 `tests/` 集成目录；`cmdline.rs:157` 真机冒烟测试依赖本机 LeagueClientUx（CI 上恒 skip，实际覆盖 0）。
- CI 面全（typecheck → build → fmt --check → clippy -D warnings → test → tauri build 冒烟），但 `cargo test` 未加 `--locked`；无 cargo-deny/cargo-audit/npm audit。
- 版本：`Cargo.toml` = `tauri.conf.json` = `frontend/package.json` = 1.0.7 一致；但 `release.yml` 「Inject version」不更新 `frontend/package.json`，长期会漂移。
- `tokio` features 用 `full`（过重）；reqwest 默认 native-tls 与 rustls 双 TLS 栈并存。

---

## 四、Minor（择要）

| 问题 | 位置 |
|------|------|
| 英文注释残留（违反注释中文规范） | `main.rs:1`、`updateStore.ts:89`、「Wails」过时注释 `api.ts:11`、`backend.ts:15` |
| `WailsFn` 迁移遗留命名 | `frontend/src/lib/backend.ts:16` |
| 魔法数字：rating 权重 `1.1/1.2/0.6`、`86400`、防抖 400/600/800ms、胜率 45/55、KDA 2.5/4.0 | `lcu_match.rs:531-541`、`logging.rs:52`、`gameinfoStore.ts:145`、`PlayerSlotCard.tsx:140` |
| 公共 API `///` 文档覆盖率 36%（50/137），`history.rs` 19 个 pub fn 全裸 | `service/history.rs`、`service/gameinfo.rs` |
| 逻辑 unwrap（重构即炸） | `parser/lcu_match.rs:484` |
| 静态正则每次调用重新编译 10 条 | `update.rs:60-103` |
| `page.clamp(0, 1_000_000)`、`uniq.len() >= 40`、`id.len() >= 32` 裸阈值 | `history.rs:366,498,846` |
| 日志行无时间戳、跨午夜不轮转 | `logging.rs:17-20,62` |
| `Monitor::spawn` 轮询任务无取消句柄，`stop()` 并不停止循环（注释不实） | `monitor.rs:251-270` |
| 会话代次 TOCTOU 窗口 | `monitor.rs:119-125` |
| 更新成功路径 `is_downloading` 不复位 | `update.rs:287-290` |
| 强退兜底含无效语句 `let _ = Store::global().get();` | `update.rs:306-311` |
| 响应体大小限制在全量读入内存后才检查 | `client.rs:78-85`、`sgp/mod.rs:120-125` |
| 便携 helper 不校验 `target_exe` 归属（同用户权限，卫生问题） | `portable_updater.rs:271-291` |
| CI 中 signer 密码经命令行传入（进程列表可见） | `release.yml` Package step |
| `frontend/package.json.md5` 残留垃圾文件 | 前端根目录 |
| 死别名 `fetchRanked` 全库无引用 | `frontend/src/features/history/api.ts:47` |
| 双端各补一次 5 槽（后端 `build_slots` + 前端 `normalizeView`） | `gameinfo.rs:922`、`gameinfoStore.ts:53` |
| `pick_participant` 找不到本人时静默回落 `participants[0]`（展示路人数据无标记） | `parser/lcu_match.rs:612` |
| SGP 适配丢弃段位字段，`tier_short` 恒空 | `parser/sgp_match.rs:208` |

**关于「Cargo.toml 乱码」**：经字节级验证，`Cargo.toml` 与全部 `.rs` 均为合法 UTF-8，`description` 码点完好。显示的「自定�?」是查看端按 GBK/cp936 解读 UTF-8 造成的显示层错位，**不是文件损坏**。修复方式是统一编辑器/终端为 UTF-8（代码页 65001），并加 `.editorconfig`（`charset = utf-8`）固化约定；切勿「转存修复」，反而会写坏文件。

---

## 五、亮点（值得保持）

1. **测试文化**：zip 路径穿越拒绝、更新回滚、Monitor 状态机、FakeHttp 服务层全链路都有专项测试。
2. **更新器加固**：minisign 签名校验 → 防穿越解压 → rename 回滚，链条完整。
3. **LCU 路径白名单**（`PATH_PREFIX_ALLOWLIST` + `path_clean` + `%2e`/反斜杠拒绝）实现质量好且有测试。
4. **CI 强制 `-D warnings`** + fmt/clippy/test/build 冒烟，`cargo check` 零警告、无 dead code。
5. **架构分层健康**：`commands → service → lcu/liveclient/sgp` 单向依赖，trait 注入（`LcuHttp/HistApi/LiveApi/LcuGetter`）消除反向依赖，无循环依赖；`sgp` 零 LCU 依赖是亮点。
6. **无敏感信息泄漏**：已逐点核对全部 `log!` 调用，token/密码/SGP token 未入日志。
7. **前端类型纪律**：零 `any`、strict 全开、竞态防护（请求序号/alive）细致。

---

## 六、改进建议（优先级排序）

### P0（立即）
1. **修复 C1**：`Monitor::tick` 探测移出锁外；`ConnStatus` 拆独立 `RwLock` 快照；去掉 `lib.rs:177` 退出路径 `block_on`。同时消除「UI 假死 + 退出挂起 + 潜伏死锁」三症状。
2. **修复 C2**：`is_remake` 单点化；**修复 C3**：`MatchPage` 拆 `total: Option<i32> + has_more: bool`。

### P1（本迭代）
3. **收紧安全面**：移除/默认停用 `ghp.ci` 镜像（或双源交叉验证 + 拒绝低于 current 的版本）；删除 `Info.sha256` 死字段；配置最小 CSP（`default-src 'self'; img-src 'self' data:; style-src 'self' 'unsafe-inline'`）；`Credentials` 手写 Debug 掩码 token。
4. **统一错误类型**：以 `LcuError` 为样板建全局 `AppError`（thiserror），命令边界转用户文案；消灭 `contains("404")` 这类字符串判语义。
5. **前端止血**：gameinfo 刷新失败补错误态（替换静默 catch）；每个视图包一层 ErrorBoundary；消除 `<button>` 嵌套交互元素 + `UpdateDialog` 补 dialog 语义。

### P2（下个迭代）
6. **拆分巨型单元**：`parse_match_detail`（223 行）拆 4 段；`gameinfo.rs` 拆 `gameinfo/{model,lobby,champ_select,live,career,cache}.rs`；`history.rs` 拆出 `asset_cache`/`sgp_tokens`；前端拆 `MatchDetailPanel`/`PlayerSlotCard` 并把 `AssetImg`、相对条、`Tone`、`toSummonerResult` 上移 `lib/`。
7. **统一可空性与派生口径**：视图模型统一 `Option`（或统一哨兵，二选一）；`queue_filter [-1]`/`summoner_id=="0"`/「未定级」改 Option/枚举；`PlayerKey::{SummonerId,Puuid}` 替代启发式判别；队列/phase/tier 文案单点化（后端导出或构建期生成 TS 常量）。
8. **并发放量治理**：`join_all` 加 `Semaphore` 上界；SGP 加闸门；缓存加 single-flight；`queue_table` 改 `LazyLock` 或 `match` 跳转表。

### P3（持续）
9. **补工程化底座**：写 README（构建/发版/Windows-only）；前端加 ESLint（`eslint-plugin-react-hooks`）+ 最小测试；CI 加 `--locked`、cargo-audit；release 版本注入补 `frontend/package.json`；删死字段（`self_team_index`/`comp_score`/未消费的 `MatchSummary` 字段）；拆 `page_size`/`career_limit`；`rating`/`killPct` 改误导命名。
10. **规范化收尾**：`.editorconfig`（charset=utf-8）；补齐服务层中文 `///`（36%→80%+）；提取重复工具（`flex_str`/`path_escape`/页大小校验）到 `util`；rating 权重、时间阈值具名常量化。

---

## 七、分维度评分速查

| 维度 | 得分 | 一句话 |
|------|------|--------|
| Rust 命名 | 9 | 高度合规 |
| Rust 注释/编码 | 8 | 中文注释质量好；「乱码」是显示层问题 |
| Rust 错误处理 | 5.5 | thiserror 孤岛 |
| 函数长度/复杂度 | 5 | 5 个超长函数 + 10 层嵌套 |
| DRY | 6 | flex/path_escape/页大小校验重复 |
| 魔法数字 | 6.5 | 评分权重与时间阈值裸奔 |
| 文档覆盖 | 4 | 服务层全裸 |
| dead code | 9.5 | cargo check 0 warning |
| 模块组织 | 7.5 | 分层清晰、个别文件偏胖 |
| TS 类型安全 | 8 | 零 any；IPC 断言体系扣分 |
| React 惯例 | 7 | hooks 规范；1 处依赖压制 |
| 前端文件规模 | 6 | 两个大组件该拆 |
| 前端状态管理 | 6 | 三轨并行是最大架构债 |
| 前端 a11y | 6 | 有意识但有嵌套交互硬伤 |
| 数据结构设计 | 6.5 | 契约对应度高、可空性双轨 |
| 并发安全 | 5 | C1 硬伤；闸门实现本身合格 |
| 安全 | 6.5 | 更新器强、CSP/镜像是债 |
| 工程化 | 6 | CI 全、缺 README/前端 lint |
