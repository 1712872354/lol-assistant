# 无障碍合规审计：战绩 & 对局信息界面

> **修复状态（2026-10-07 晚，两批全部完成）**：17 项问题**全部闭环**。第一批（#1–#15、#17）经 `tsc` + vitest（15/15）+ 生产构建验证；第二批补齐遗留：**#16 路由焦点迁移**（AppLayout 主内容区 `tabIndex=-1` + 路由切换 focus + sr-only 页面播报）；**#5 名称替代文本升级为逐件 alt**（后端新增 `get_match_asset_names` 命令，从 LCU items/spells/perks/augments JSON 提取 `id→name` 并缓存，前端 `useAssetNames` hook 批量查询，装备/召唤师技能/符文逐件朗读真实名称，未就绪时"装备/召唤师技能/符文"兜底）。英雄图标仍按装饰处理（旁边有玩家名/KDA 文本上下文）。验证：`cargo check` ✓ + `tsc` ✓ + vitest 15/15 ✓ + `vite build` ✓。

- **审计日期**：2026-10-07
- **标准**：WCAG 2.2 AA（对比度 1.4.3 / 非文本 1.1.1 / 键盘 2.1.1 / 焦点 2.4.7 / 状态消息 4.1.3 等）
- **范围**：`features/history`（战绩页）+ `features/gameinfo`（对局信息页），含 `lib/AssetImg`、`lib/tone`、`index.css` 色板
- **方法**：代码走查 + 色板对比度实测（sRGB 相对亮度算法，按 alpha 合成实际背景）

## 结论

共发现 **17 项问题：P0 阻断 3 项、P1 严重 8 项、P2 改进 6 项**。三大硬伤：

1. **键盘用户无法排序对局表格**（排序绑定在非交互 `th` 的 onClick 上）；
2. **原生 `<button>` 全部没有可见焦点样式**（全局 CSS 未定义 `:focus-visible`，仅 shadcn Button/Input 自带）；
3. **标签页内嵌套 `<button>`**（非法 HTML，读屏/键盘行为未定义）。

对比度方面语义色（胜负/金色/红绿）大多达标，但 **`muted-foreground` 的透明度变体（/50–/70）全线不达标**（最差 1.90:1），且**亮色主题多个色不达标**（brand-gold 2.92:1、amber-500 2.15:1）。

---

## 问题清单

### P0 — 阻断性（键盘/读屏用户功能不可用）

| # | 问题 | WCAG | 位置 | 现状 → 修复 |
|---|------|------|------|------------|
| 1 | 表格排序键盘不可达 | 2.1.1 | `PlayerDataTable.tsx:361-384` | 排序绑定在 `<TableHead onClick>`，`th` 不可聚焦 → 表头内渲染 `<button>` 承载排序，配 `aria-sort` |
| 2 | 原生按钮无可见焦点 | 2.4.7 | `MatchCard.tsx`、`PlayerSlotCard.tsx`、`PlayerDataTable.tsx`（玩家名按钮）、`SummonerTabs.tsx`（关闭按钮）、`ViewControls.tsx` | `index.css` 无 `:focus-visible` 规则，Tab 导航无任何焦点提示 → 加全局焦点环 |
| 3 | button 嵌套 button | 4.1.1 | `SummonerTabs.tsx:103-127` | `TabsTrigger`（Radix 渲染为 `button[role=tab]`）内部嵌关闭 `<button>`，非法 DOM → 关闭按钮移出触发器，绝对定位平级摆放 |

### P1 — 严重（合规硬伤，应尽快修）

| # | 问题 | WCAG | 位置 | 现状 → 修复 |
|---|------|------|------|------------|
| 4 | 仅靠颜色传达信息 | 1.4.1 | `PlayerDataTable.tsx:399-403`（本人行仅金色阴影+底色）、`ThreatBar.tsx`（好/中/差仅绿黄红）、`PlayerSlotCard.tsx:132-153`（胜率/KDA 档位色） | 本人行加 `sr-only`「（本人）」；ThreatBar/色档数值附档位文字（可见或 sr-only） |
| 5 | 信息型图像无替代文本 | 1.1.1 | `AssetImg.tsx:104-112` 恒定 `alt=""`；装备 7 格、召唤师技能、符文、近况英雄均为信息载体 | `AssetImg` 增加 `alt` prop；装备/技能/符文传真实名称，纯装饰传空 |
| 6 | 仅 `title` 提供说明 | 1.1.1 / 2.1.1 | `MatchDetailPanel.tsx:231-245`、`PlayerDataTable.tsx:374`（列头 hint）、`PlayerSlotCard.tsx:167`、`RankChip.tsx`、`TeamPanel.tsx:33-43` | `title` 触屏/键盘不可靠 → 改 Tooltip（可聚焦触发）或 sr-only 文本 |
| 7 | 无标题层级 | 1.3.1 / 2.4.6 | 两个界面全程 `div/span`，队头「胜利/失败」、`TeamPanel` 队名均为 span | 队头用 `<h3>`，列表/面板标题用 `<h2>` |
| 8 | 状态消息不播报 | 4.1.3 | 全部骨架屏无 `aria-hidden`/`role="status"`；错误块无 `role="alert"`（`MatchList.tsx:159`、`MatchDetailPanel.tsx:159`、`GameInfoView.tsx:52`）；复制成功无反馈 | 骨架屏 `aria-hidden` + sr-only「加载中」；错误块 `role="alert"`；复制后 live region 播报 |
| 9 | ARIA 角色误用 | 4.1.2 | `ViewControls.tsx:33-65`：`role="tablist"/"tab"` 但无 `aria-controls`、无 roving tabindex、无方向键 | 改 `radiogroup` + `aria-checked`（筛选语义更贴切），或补全 tabs 键盘模式 |
| 10 | 对比度不足 | 1.4.3 | `text-muted-foreground/50–/70` 全线；亮色 `brand-gold`、`amber-500`、`win-fg on win-bg`（实测见下表） | 透明度变体下限 /75；亮色换 `--brand-gold-dim`、`amber-700`；详见「对比度实测」 |
| 11 | 字号过小 | 1.4.4 | 9px（MVP/ACE 徽标）、10px（shortTime、duration、tag、评分、胜率标签、等级角标、UNRANKED 等） | 文字下限 11px，关键数据 ≥12px；与低对比叠加处优先处理 |

### P2 — 改进项

| # | 问题 | WCAG | 位置 | 修复 |
|---|------|------|------|------|
| 12 | 可访问名冗长/缺失/重复 | 4.1.2 / 2.4.6 | `PlayerSlotCard` 头部大按钮名=整卡文本；`ThreatBar` 的 Progress 无名称；搜索框仅 placeholder（`SummonerTabs.tsx:154`）；关闭标签按钮全部叫「关闭标签」 | 按钮加简洁 `aria-label`；Progress 加 `aria-label`；Input 加 `aria-label`；关闭按钮 `aria-label={\`关闭 ${t.name}\`}` |
| 13 | 装饰符被朗读 | 1.1.1 | 多处 `<span className="opacity-30">·</span>`、KDA 的 `/` | 加 `aria-hidden` |
| 14 | 选中状态无语义 | 4.1.2 | `MatchCard` 的 `active` 仅视觉边框/阴影 | 选中卡片加 `aria-current="true"` |
| 15 | 动画无减弱降级 | 2.3.3 | `skeleton-shimmer`、`animate-spin`、`seg-indicator`、`press-scale` | 全局 `@media (prefers-reduced-motion: reduce)` 关闭 |
| 16 | 路由后焦点不迁移 | 2.4.3 | `PlayerSlotCard` 点「战绩详情」跳 `/history` 后焦点残留 | 路由切换后焦点移到新页容器 |
| 17 | 对局错误条无角色 | 4.1.3 | `GameInfoView.tsx:52` 刷新失败提示 | `role="alert"` |

---

## 对比度实测（正文阈值 4.5:1；大文本/UI 组件 3:1）

暗色主题（默认）：

| 用途 | 色值 | 对比度 | 判定 |
|------|------|--------|------|
| shortTime 10px `muted-foreground/50` | #8b95a8 @50% | 2.43:1 | ❌ |
| duration 10px `/60` | #8b95a8 @60% | 2.94:1 | ❌ |
| KDA 副行 10px `/65` | #8b95a8 @65% | 3.23:1 | ❌ |
| tag 11px `/70` | #8b95a8 @70% | 3.54:1 | ❌ |
| 表头 11px `/85` | #8b95a8 @85% | 4.61:1 | ✅ |
| win-fg on card | #2dd4a8 | 9.39:1 | ✅ |
| loss-fg on card | #e06b75 | 5.53:1 | ✅ |
| brand-gold 段位 11px | #c8aa6e | 7.98:1 | ✅ |
| 错误文字 | #e06b75 on #080b12 | 6.12:1 | ✅ |

亮色主题：

| 用途 | 色值 | 对比度 | 判定 |
|------|------|--------|------|
| `muted-foreground/50` 10px | #6b7a90 @50% | 1.90:1 | ❌ |
| `muted-foreground/60` | #6b7a90 @60% | 2.21:1 | ❌ |
| `muted-foreground/65` | #6b7a90 @65% | 2.39:1 | ❌ |
| `muted-foreground/70` | #6b7a90 @70% | 2.59:1 | ❌ |
| `muted-foreground` 全量 | #6b7a90 | 4.36:1 | ⚠️ 边缘 fail |
| 段位 `brand-gold` | #b8922e | 2.92:1 | ❌ |
| 评分 `brand-gold/15` 底 | #b8922e | 2.54:1 | ❌ |
| 伤转 mid `amber-500` | #f59e0b | 2.15:1 | ❌ |
| 胜 `win-fg on win-bg` | #0d8a68 on #e8f8f2 | 3.94:1 | ❌ |
| 负 `loss-fg on loss-bg` | #c43c48 on #fdeced | 4.50:1 | ✅ |
| 错误文字 | #d14b56 on #f0f2f6 | 3.86:1 | ❌ |

---

## 修复方案（含代码）

### 1. 全局焦点环（修 #2，index.css）

```css
/* 键盘焦点可见：原生 button/链接/输入统一焦点环 */
:focus-visible {
  outline: 2px solid var(--ring);
  outline-offset: 2px;
  border-radius: inherit;
}
```

### 2. 排序表头按钮化 + aria-sort（修 #1，PlayerDataTable.tsx）

```tsx
<TableHead aria-sort={sorted === "asc" ? "ascending" : sorted === "desc" ? "descending" : "none"}>
  {canSort ? (
    <button
      type="button"
      onClick={header.column.getToggleSortingHandler()}
      className="flex w-full items-center justify-end gap-0.5 px-1 py-1 hover:text-foreground"
      aria-label={`按${String(label)}排序`}
    >
      {label}
      <span aria-hidden className="text-[9px] opacity-70">
        {sorted === "desc" ? "▼" : sorted === "asc" ? "▲" : "↕"}
      </span>
    </button>
  ) : (
    label
  )}
</TableHead>
```

### 3. 标签页关闭按钮移出触发器（修 #3，SummonerTabs.tsx）

```tsx
<div key={t.id} className="relative shrink-0">
  <TabsTrigger value={t.id} className="...pr-6">
    <AssetImg ... />
    <span className="truncate">{name}</span>
  </TabsTrigger>
  <button
    type="button"
    aria-label={`关闭 ${name}`}
    onClick={() => closeTab(t.id)}
    className="absolute right-1 top-1/2 -translate-y-1/2 rounded p-0.5 ..."
  >
    <X className="h-3 w-3" />
  </button>
</div>
```

### 4. 本人行/胜负色档补非颜色编码（修 #4）

```tsx
{/* PlayerDataTable TableRow 内 */}
{self && <span className="sr-only">（本人）</span>}

{/* ThreatBar：档位进可访问名 */}
<Progress aria-label={`${label ?? "相对值"} ${Math.round(value)}%，${toneText[tone]}`} ... />
// toneText = { good: "较高", mid: "中等", bad: "较低", muted: "" }
```

### 5. AssetImg 支持替代文本（修 #5）

```tsx
interface AssetImgProps {
  kind: string;
  id: number;
  size?: number;
  className?: string;
  title?: string;
  /** 信息型图像传真实名称（装备/技能/英雄）；纯装饰留空 */
  alt?: string;
}
// <img ... alt={alt ?? ""} />
```

装备格调用处改为 `<AssetImg kind="item" id={id} alt={itemName(id)} ... />`（名称映射可由 `GetMatchAsset` 后端返回或前端静态表）。

### 6. 队头标题化（修 #7，MatchDetailPanel.tsx / TeamPanel.tsx）

```tsx
<h3 className="text-[14px] font-bold tracking-wide">{result}</h3>
```

### 7. 阵营筛选改 radiogroup（修 #9，ViewControls.tsx）

```tsx
<div role="radiogroup" aria-label="阵营筛选" className="...">
  {SIDE_FILTERS.map((f) => (
    <button
      key={f.value}
      type="button"
      role="radio"
      aria-checked={on}
      onClick={() => setSideFilter(f.value)}
      className="..."
    >
      {f.label}
    </button>
  ))}
</div>
```

### 8. 加载/错误状态（修 #8、#17）

```tsx
{/* 骨架屏外层 */}
<div aria-hidden="true">{/* skeleton */}</div>
<p className="sr-only" role="status">正在加载对局明细…</p>

{/* 错误块 */}
<div role="alert" className="...">对局明细加载失败…</div>
```

### 9. 亮色色板修正（修 #10，index.css `[data-theme="light"]`）

```css
--brand-gold: #8a6c1e;        /* 段位/评分文字：2.92→5.7:1 */
--muted-foreground: #5c6a80;  /* 4.36→5.2:1 */
--win-fg: #0a7055;            /* win-bg 上 3.94→5.3:1 */
--destructive: #b13742;       /* 背景上 3.86→5.1:1 */
/* amber-500 伤转 mid 亮色改 amber-700 (#b45309) */
```

组件内透明度变体统一抬到 /75 以上：`/50→/75`、`/60→/75`、`/65→/80`、`/70→/80`。

### 10. 动画降级（修 #15，index.css）

```css
@media (prefers-reduced-motion: reduce) {
  *, *::before, *::after {
    animation-duration: 0.01ms !important;
    animation-iteration-count: 1 !important;
    transition-duration: 0.01ms !important;
  }
}
```

---

## 文件级修复清单

| 文件 | 涉及问题 | 改动量 |
|------|----------|--------|
| `frontend/src/index.css` | #2 #10 #11 #15 | 焦点环 + 亮色色板 + reduced-motion |
| `frontend/src/features/history/PlayerDataTable.tsx` | #1 #4 #5 #6 #11 | 排序按钮、sr-only 本人、alt、字号 |
| `frontend/src/features/history/SummonerTabs.tsx` | #3 #12 #13 | 关闭按钮重构、aria-label、装饰符 |
| `frontend/src/features/history/MatchCard.tsx` | #2 #11 #14 | 焦点环、字号、aria-current |
| `frontend/src/features/history/MatchDetailPanel.tsx` | #6 #7 #8 #13 | 队头 h3、alert、装饰符 |
| `frontend/src/features/history/MatchList.tsx` | #8 #12 | alert、页码播报 |
| `frontend/src/features/gameinfo/PlayerSlotCard.tsx` | #2 #4 #6 #12 | 焦点环、色档文字、按钮命名 |
| `frontend/src/features/gameinfo/ViewControls.tsx` | #9 | radiogroup |
| `frontend/src/features/gameinfo/TeamPanel.tsx` | #6 #7 | 标题化、StatChip 说明 |
| `frontend/src/features/gameinfo/ThreatBar.tsx` | #4 #12 | aria-label + 档位 |
| `frontend/src/features/gameinfo/GameInfoView.tsx` | #8 #17 | role="alert" |
| `frontend/src/lib/AssetImg.tsx` | #5 | alt prop |
| `frontend/src/features/gameinfo/RecentMatchList.tsx` | #13 | 装饰符 aria-hidden |
