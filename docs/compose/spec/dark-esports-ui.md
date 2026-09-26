---
feature: dark-esports-ui
status: delivered
updated: 2026-09-26
branch: master
commits: 7c2600d..7c2600d # 工作区未提交改动（基于 7c2600d 的 dirty tree）
---

# 暗色电竞工具风改造（战绩 + 对局）

## Report

**What was built** — 将 LOL 助手主业务两页从 shadcn 亮色 admin 形态改造为暗色电竞工具风：深蓝黑底 + Hextech 金强调 + 青绿/绯红胜负 + 冷蓝/暖红敌我。战绩页列表用左 3px 色轨 + 加粗 KDA 扫读，明细表数字升为 14–15px 主角、相对条降为 3px 细条，本人行金边，MVP 金标 / ACE 银标，队头为记分牌条。对局页侦察卡以胜率/均 KDA 大数字为核心，自卡金轨，段位与评分统一金/冷蓝，去掉品红与米金旧语义。全局 tokens 默认暗色，`[data-theme="light"]` 保留备选；`index.html` 静态默认 `dark` 防白闪。

**Verification** —
- `pnpm --dir frontend typecheck` → PASS
- `pnpm --dir frontend test` → PASS（6 files / 15 tests）
- `pnpm --dir frontend build` → PASS（既有 chunk size 警告，PRE-EXISTING）
- 独立审查 general-1 → APPROVE（无 critical）；3 个 non-blocking nit 已修：MatchCard 选中态左轨类冲突、`index.html` 默认 dark、伤转三档色

**Journey log** —
1. 仓库 master 上已有未提交 shadcn 迁移，按用户选择在当前工作区叠加视觉升级，未另开 worktree。
2. `tone.ts` / 部分中文源文件在 PowerShell 下显示乱码，编辑时用 ASCII 类名锚点或整文件重写避免误匹配。
3. Tailwind `cn()` 不会按 class 顺序解决同属性冲突（`border-l-win-bar` vs `border-l-selected-ring`），选中态改为互斥 class。
4. 审查指出 `index.html` 的 `data-theme` 是 JS `applyTheme` 前的真相源，默认改 dark 消除闪烁。
5. 威胁色档约定为青绿/琥珀/绯红三档；伤转原先只有好坏两档，已补中档。

## [S1] Problem

当前 UI 是 shadcn zinc 亮色 admin 形态，与产品定位（夜间使用、对局分析、扫读数据的电竞工具）不匹配：

1. **气质错位**：浅灰底 + 紫色 accent 像 SaaS 后台，不像 LOL 伴随工具（对照 OP.GG / Blitz / 客户端 chrome）。
2. **色相过载**：紫选中、金段位、绿/红胜负、蓝/粉队伍、品红评分、绿伤转——6+ 色系抢注意力，数值反而退居二线。
3. **数字不够「主角」**：列表/明细大量 10–11px，KDA、伤害、评分扫读费力；相对条与数字同级，增加噪声。
4. **卡片壳层过重**：圆角卡片套圆角卡片 + shadow，电竞工具应有的「数据台」密度被稀释。
5. **两页语义不统一**：战绩页紫选中 / 对局页米金自卡 / 蓝粉阵营，视觉语言碎片化。

## [S2] Design

### 风格锚点

**OP.GG 暗色数据台 × 英雄联盟 Hextech chrome × 转播记分牌**。夜间友好、对比强、数字优先；装饰只服务语义，不做「好看但无关」的色块。

### 调色板（暗色优先）

| 角色 | Token | 值 | 用途 |
|------|-------|-----|------|
| 底 | `--background` | `#0A0D14` | 全局深蓝黑 |
| 面 | `--card` / `--popover` | `#121824` | 列表卡、表体、浮层 |
| 面抬升 | `--sidebar` / muted | `#0F141E` / `#1A2230` | 侧栏、行 hover、条槽 |
| 墨 | `--foreground` | `#E8ECF4` | 主文案 |
| 次墨 | `--muted-foreground` | `#8B95A8` | 标签、时间、次级统计 |
| 线 | `--border` | `#243044` | 分割与描边（细、低对比） |
| 品牌金 | `--brand-gold` | `#C8AA6E` | 本人高亮、主强调、选中环 |
| 胜 | `--win-*` | 青绿 `#2DD4A8` 系 | 胜利、正向数值 |
| 负 | `--loss-*` | 绯红 `#E06B75` 系 | 失败、威胁/负向 |
| 无效 | `--remake-*` | 中性灰 | 重赛/无效 |
| 我方 | `--ally-*` | 冷蓝 `#4A9EDE` 系 | 对局页我方 |
| 敌方 | `--enemy-*` | 暖红 `#E06B75` 系 | 对局页敌方 |
| 评 | `--rating-bg` | 低饱和金 | 综合评分 chip（非品红） |

亮色主题保留但降级为备选（`[data-theme="light"]`）。

### 字体与层级

- 字体栈不变（Segoe UI / 微软雅黑 UI），`tnum` / `stat-num` 全量用于统计。
- **数字主角**：列表 KDA `18px`，明细 KDA/伤害/金钱/评分 `14–15px`，对局 WR `22px` / KDA `20px`。
- 标签 `11px`，元信息 `10–11px` muted；玩家名 `13px/600`。
- 对比靠字重与尺寸，不靠更多颜色。

### 布局与密度

- 保持主从结构：左战绩列表（280px）+ 右明细；对局页双队面板。
- **去重壳**：外层 `rounded-lg border`，不再叠 shadow/大圆角。
- 相对条降为 **3px** 细条。

### 语义色规则（统一两页）

| 语义 | 做法 |
|------|------|
| 胜/负/无效 | 左侧 3px 色轨 + 文字色；**禁止**大面积色底铺满 |
| 本人 | 金色左边框 / inset 金轨 + 极低透明金底；选中列表项 = 金环 |
| 我方/敌方 | 表头/面板左边框用 ally/enemy 色；表体近中性 |
| 威胁（WR/KDA/伤转） | 好/中/差 三档：青绿 / 琥珀 / 绯红 |
| MVP / ACE | 金标 / 银灰标，小号大写字母 |

### 签名时刻

1. **列表左轨扫读**：胜负色轨 + 粗体 KDA 一眼定局势。
2. **记分牌式队头**：结果字 + 侧别 + 队伍 K/D/A · 经济 · 时长。
3. **明细数字台**：大号 tabular 数字列 + 细相对条，金边标出本人。
4. **对局侦察卡**：WR% 与均 KDA 作大数字，威胁条压在下方。

### 组件级约定

- `MatchCard`：左轨 3px、结果文字右上、KDA 居中加粗、其余 muted；选中金轨与胜负轨互斥。
- `PlayerDataTable`：列顺序 玩家 | KDA | 伤害 | 金钱 | 伤转 | 装备 | 评分。
- `PlayerSlotCard`：暗面卡片，自卡金轨；WR/KDA 大数字；评分 chip 金系。
- `AppSidebar` / `TitleBar`：随 tokens 暗化；导航激活态金/冷蓝。

## [S3] Out of Scope

- 设置页深度重做（仅被 tokens 被动影响）。
- 后端字段 / 评分公式 / 排序逻辑变更。
- 新图表组件、主题切换器 UI 重做、多主题系统。
- 亮色主题的精修（保留可编译即可）。

## Tasks

- [x] T1: 重写 `index.css` 暗色 tokens + 语义色 + 字号/轨道工具类 — acceptance: 暗色下两页主界面色相收敛为金/青绿/绯红/冷蓝，无紫色主导；`pnpm typecheck` 通过 (covers: S2)
- [x] T2: 战绩页列表与明细表按签名时刻改造（MatchCard / PlayerDataTable / MatchDetailPanel / badges） — acceptance: 列表左轨胜负可扫读；明细数字≥14px 且相对条降噪；本人金边；胜负仅色轨+文字 (covers: S2; depends: T1)
- [x] T3: 对局页侦察卡与队伍面板统一（PlayerSlotCard / TeamPanel / ThreatBar / RankChip） — acceptance: 与战绩页同一套 token 与威胁色；自卡金轨；评分/生涯 chip 改用 rating/career 金系；去掉品红/米金旧语义 (covers: S2; depends: T1)
- [x] T4: 壳层微调（AppSidebar 激活态、TitleBar、滚动条）并对齐两页间距节奏 — acceptance: 侧栏无紫块；两页容器边框/圆角/间距一致 (covers: S2; depends: T1)
- [x] T5: 验证 typecheck / test / build，并做视觉走查记录 — acceptance: 三条命令全绿；走查问题关闭或记入报告 (covers: S2; depends: T2, T3, T4)
- [x] T6: 独立审查 diff 与验收标准 — acceptance: 无 critical；报告写入本文档 (covers: S2; depends: T5)
