# 发版步骤（GitHub Actions · Tauri）

仓库需 **Public**（国内用户才能走镜像下载 Release 资产）。

## 0. 前置：更新签名密钥（仅首次/轮换）

1. 本地生成：`cargo tauri signer generate --ci -w %USERPROFILE%\.tauri\lol-assistant.key`
2. 把公钥写入 `src-tauri/tauri.conf.json` → `plugins.updater.pubkey`（已完成）
3. GitHub 仓库 Secrets 配置：
   - `TAURI_SIGNING_PRIVATE_KEY`：私钥文件**全文**（`~/.tauri/lol-assistant.key`）
   - `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`：私钥密码（无密码则留空/设为空字符串）

**私钥勿提交仓库。**

## 1. 日常发版

1. 更新 `CHANGELOG.md`：新增 `## [vX.Y.Z] - 日期`，整理本次改动
2. 确认本地全绿：

```powershell
# 前端
cd frontend; pnpm typecheck
# Rust（在 src-tauri 下）
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test
```

3. 提交后打 Tag 并推送：

```powershell
git tag -a vX.Y.Z -m "vX.Y.Z"
git push origin main --tags
```

4. `release.yml` 会自动：
   - 注入版本号到 `Cargo.toml` + `tauri.conf.json`
   - `pnpm tauri build`（前端构建 + NSIS + updater 签名产物 `.sig`）
   - 打包 `LOLAssistant-Portable-<ver>.zip`（根目录 `LOLAssistant-portable/` + `portable.flag`）并签名
   - 生成 `latest.json`（双 platform：`windows-x86_64` / `windows-x86_64-portable`）
   - 生成 `SHA256SUMS.txt`，从 `CHANGELOG.md` 摘取说明，创建 GitHub Release

5. 在 Release 页核对资产即可，**无需再手工贴描述**。

## 2. 资产命名（与应用内更新器约定一致）

| 资产 | 说明 |
|------|------|
| `LOLAssistant-Setup-<ver>.exe` (+ `.sig`) | NSIS 安装包（推荐） |
| `LOLAssistant-Portable-<ver>.zip` (+ `.sig`) | 绿色版，根为 `LOLAssistant-portable/` |
| `latest.json` | `tauri-plugin-updater` 清单（双 endpoint 拉取） |
| `SHA256SUMS.txt` | 人工核对哈希（更新链路已改 minisign，不再依赖正文 SHA256） |

## 3. 更新链（raw.githubusercontent.com 加速）

- **加速源（优先）**：`https://raw.githubusercontent.com/1712872354/lol-assistant/dist/latest.json`
  - 发布工作流会把 `latest.json` + 制品（exe/zip + .sig + SHA256SUMS）发布到 `dist` 分支（孤儿单提交覆盖，不累积历史）；
    清单内制品 URL 也走 raw，整条更新链在 raw 可达时均为官方域 CDN 加速
- **官方回退**：`https://github.com/1712872354/lol-assistant/releases/latest/download/latest.json`
  - raw 不可达时自动回退 GitHub Release（清单内制品 URL 走 releases/download）
- **端点只允许 GitHub 官方域**（`github.com` / `raw.githubusercontent.com`），更新器测试有白名单断言把关；
  制品仍经 minisign 验签 + 版本单调性校验（拒绝降级），与下载源无关
- 注意：端点表编译进程序，**本变更自下个版本起生效**

## 4. 本地构建

```powershell
# 开发
pnpm tauri dev
# 发布产物（需已配置签名 Secrets 环境变量，否则 updater 产物可能不完整）
pnpm tauri build
```

## 5. 已知限制

- 未做 Authenticode 签名，杀软可能提示；更新链路用 minisign 校验
- **≤1.0.6 旧客户端**走旧 SHA256 自写链路，无法读到 `latest.json`——需**最后一次手动安装 ≥1.0.7**，之后应用内更新正常（见 CHANGELOG）

## 6. 若 fork 仓库

同步修改：

- `src-tauri/tauri.conf.json` → `plugins.updater.endpoints`
- `src-tauri/src/update.rs` → `REPO_OWNER` / `REPO_NAME`
- `.github/workflows/release.yml` → `latest.json` 内 owner/repo
