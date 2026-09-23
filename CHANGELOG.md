# 更新日志

本文件对应 GitHub Release 说明。发版时把对应小节正文贴到 Release 描述即可（或由 Actions `generate_release_notes` 补充提交列表）。

## [v1.0.6] - 2026-09-23

### 修复

- **更新提示「缺少 SHA256，拒绝安装未校验安装包」**：`SHA256SUMS.txt` 直连 `github.com` 国内常超时导致哈希为空
  - 优先从官方 Release 正文解析哈希（CI 发版时把 `SHA256SUMS` 写入正文，随 API 一次返回）
  - 回退经 `api.github.com` 资产端点拉取 checksums（该域名国内通常可达）
  - 下载镜像列表补充 `gh-proxy.com`

### 说明

- **手动安装过一次后，后续应用内更新即可正常校验**（旧版客户端仍只拉 `github.com` 直链，无法热修）
- 安装包 `LOLAssistant-Setup-1.0.6.exe` / 绿色版 `LOLAssistant-Portable-1.0.6.zip` / `SHA256SUMS.txt`，Windows x64 · 需 WebView2 运行时

---

## [v1.0.5] - 2026-09-23

### 修复

- **系统托盘假死**：菜单项与点击消费循环先于 `SetIcon` 就绪；点击处理异步执行并恢复 panic，避免后续点击被库丢弃导致卡死
- **选人阶段己方少显示 1 人战绩**：`myTeam` 身份不全条目被过滤或 session 未到齐时，从 gameflow 花名册补回己方成员（敌方仍不补，防盲选泄露）

### 性能 / 重构

- LCU 请求并发固定闸门 2（`internal/lcu`），移除设置项「API 并发」及相关前后端绑定
- SGP token 单飞缓存（5 分钟有效 / 15 分钟错误退避），减少选人阶段重复换票

### 说明

- 安装包 `LOLAssistant-Setup-1.0.5.exe` / 绿色版 `LOLAssistant-Portable-1.0.5.zip` / `SHA256SUMS.txt`，Windows x64 · 需 WebView2 运行时

---

## [v1.0.4] - 2026-09-22

### 修复

- **对局页头像改为本局所选英雄优先**：选人锁定后 / 游戏中显示所选英雄头像；房间内或未锁定时回退召唤师头像

### 说明

- 本版本包含 [v1.0.3] 全部内容（更新器校验加固、前端竞态修复等）
- 安装包 `LOLAssistant-Setup-1.0.4.exe` / 绿色版 `LOLAssistant-Portable-1.0.4.zip` / `SHA256SUMS.txt`，Windows x64 · 需 WebView2 运行时

---

## [v1.0.3] - 2026-09-22

### Security
- 更新器：setupURL 白名单、强制 SHA256、官方 checksums、随机临时文件、`/D=` 净化
- SGP 默认校验证书；LCU 路径规范化；PlatformID/puuid 注入面收敛
- CI 增加 `go test` / `-race` 门禁

### Fixed
- 战绩明细多标签视角串数据；对局刷新竞态；明细空值守卫
- WS 节流补发最新帧；懒初始化锁；lockfile 密码含冒号
- 竞技场/多队伍不再截断 5 槽；跳页上限；事件订阅 cleanup
- 窗口与安装包中文名乱码：产物统一 ASCII `LOLAssistant.exe`

### Changed
- `build.bat` 可移植；卸载仅在确认安装目录时递归删除
- 根级 Error Boundary；配置 SetConfig 串行写入

## [v1.0.2] - 2026-09-22

### 安装

| 包 | 说明 |
|----|------|
| `LOLAssistant-Setup-1.0.2.exe` | NSIS 安装包（推荐） |
| `LOLAssistant-Portable-1.0.2.zip` | 绿色版，解压即用 |
| `SHA256SUMS.txt` | 校验哈希 |

Windows x64 · 需 WebView2 运行时

### 修复

- **本地 `build.bat` 中文乱码/命令被截断**：脚本改为纯 ASCII，cmd 不再按字节切断多字节字符
- **本地打包 `Bad text encoding`**：`build.bat` 打包前自动为 `project.nsi` 补 UTF-8 BOM（与 CI 一致）

### 安装校验

```powershell
Get-FileHash .\LOLAssistant-Setup-1.0.2.exe -Algorithm SHA256
# 与 SHA256SUMS.txt 中对应行比对
```

---

## [v1.0.1] - 2026-09-22

### 安装

| 包 | 说明 |
|----|------|
| `LOLAssistant-Setup-1.0.1.exe` | NSIS 安装包（推荐） |
| `LOLAssistant-Portable-1.0.1.zip` | 绿色版，解压即用 |
| `SHA256SUMS.txt` | 校验哈希 |

Windows x64 · 需 WebView2 运行时

### 修复

- **应用内更新安装失败**（`requires elevation`）：安装包改为 UAC 提权启动
- **更新后应用不退出**：强制退出改为原子标记 + 不依赖托盘的 `os.Exit` 兜底
- **更新说明露出原始 Markdown**：改为可读纯文本（去掉安装/校验段与表格符号）
- **安装目录不沿用已装路径**：自动识别注册表/卸载项中的安装目录，本次安装默认沿用；更新时以 `/D=` 传入

### 改进

- 暗色主题下对局页配色：大面积蓝/红色块降饱和，胜负与敌我改靠色条和文字区分，滚动条暗色化

### 安装校验

```powershell
Get-FileHash .\LOLAssistant-Setup-1.0.1.exe -Algorithm SHA256
# 与 SHA256SUMS.txt 中对应行比对
```

---

## [v1.0.0] - 2026-09-22

### 安装

| 包 | 说明 |
|----|------|
| `LOLAssistant-Setup-1.0.0.exe` | NSIS 安装包（推荐） |
| `LOLAssistant-Portable-1.0.0.zip` | 绿色版，解压即用 |
| `SHA256SUMS.txt` | 校验哈希 |

Windows x64 · 需 WebView2 运行时

### 修复

- **战绩数量真正生效**：对局页每人近况展示与统计场数跟随设置（10/20/30），不再写死 20；修改后清近况缓存并立即刷新对局页
- **API 并发改为 2 / 5 / 10 三挡**（默认 5）；旧配置 4/6/8 自动归一到 5

### 改进

- 设置文案澄清：「每页战绩数」→「战绩数量」，明确作用于对局页近况
- 明细页点击玩家名可在战绩页新标签查看该玩家战绩
- wailsjs 补齐更新相关绑定（CheckUpdate / DownloadAndInstallUpdate / GetAppVersion）

### 安装校验

```powershell
Get-FileHash .\LOLAssistant-Setup-1.0.0.exe -Algorithm SHA256
# 与 SHA256SUMS.txt 中对应行比对
```

---

## [v0.1.0] - 首个公开版本

### 安装

- Windows x64（需 WebView2，Win10 20H2+ / Win11 一般自带）
- 推荐：`LOL助手-Setup-0.1.0.exe`
- 绿色版：`LOL助手-Portable-0.1.0.zip`
- 校验：见同页 `SHA256SUMS.txt`

### 功能

- **战绩**：多召唤师标签页、分页列表、单局 10 人明细（KDA / 伤害 / 金钱 / 伤转 / 装备 / 评分、MVP/ACE）
- **对局**：房间/选人/对局阶段双队卡、近 20 场、胜率与评分对照、威胁色档
- **设置**：主题、每页战绩数、API 并发、SGP 开关、客户端路径、关闭到托盘
- **托盘**：关窗可隐藏到托盘；菜单「显示主界面 / 退出」
- **更新**：GitHub Releases 检查；支持下载 Setup → SHA256 校验 → 拉起安装

### 已知限制

- 未代码签名，部分杀软可能误报（请以 `SHA256SUMS.txt` 核对）
- 托盘图标与菜单为基础版；自启动、气泡通知后续版本再做
- 仅腾讯国服 SGP 补数完整；其它服以 LCU 为准

---

## [v0.1.4] - 2026-09-22

### 修复

- 更新安装后应用不退出、弹窗卡在「处理中…」的问题：安装器拉起后强制退出主进程，绕过「关闭到托盘」拦截
- 安装目录中文乱码（`Program Files\LOLåŠ©æ‰‹`）：`project.nsi` 改为 UTF-8 with BOM，CI 打包前自动补 BOM；默认安装目录为 `Program Files\LOL助手`

## [v0.1.3] - 2026-09-22

### 修复

- 战绩列表「下一页」在 SGP 数据源下被误禁用的问题
- 更新弹窗改为展示「本次更新」说明，而不是 GitHub 自动生成的 Full Changelog 链接

## [v0.1.2] - 2026-09-22

### 修复

- 安装包默认目录改为 `Program Files\LOL助手`

## [v0.1.1] - 2026-09-22

### 修复

- 发行资产改用 ASCII 文件名，避免 CI 中文乱码
- CI 将 NSIS 加入 PATH，确保生成安装包
