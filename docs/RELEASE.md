# 发版步骤（GitHub Actions）

仓库需 **Public**（国内用户才能走镜像下载 Release 资产）。

## 1. 日常发版

1. 更新 `CHANGELOG.md`：在文首 `## [Unreleased]` 下整理本次改动，或直接新增 `## [vX.Y.Z] - 日期`
2. 提交后打 Tag 并推送：

```powershell
git tag -a vX.Y.Z -m "vX.Y.Z"
git push origin main --tags
```

3. `release.yml` 会自动：
   - 前端构建 + `go vet` + `go test ./...`
   - 产出 `LOLAssistant-Setup-<ver>.exe` / `LOLAssistant-Portable-<ver>.zip`
   - 生成 `SHA256SUMS.txt`，并把其内容写入 Release 正文「安装校验」段（更新器从官方 API 读哈希）
   - 从 `CHANGELOG.md` 摘取说明并创建 GitHub Release

4. 在 Release 页核对资产名与哈希即可，**无需再手工贴描述**。

## 2. 资产命名（与应用内更新器约定一致）

| 资产 | 说明 |
|------|------|
| `LOLAssistant-Setup-<ver>.exe` | NSIS 安装包（推荐） |
| `LOLAssistant-Portable-<ver>.zip` | 绿色版 |
| `SHA256SUMS.txt` | 校验哈希（更新器：官方正文 → `api.github.com` 资产端点 → `github.com` 直链，镜像不采信） |

## 3. 安装校验

```powershell
Get-FileHash .\LOLAssistant-Setup-<ver>.exe -Algorithm SHA256
# 与 SHA256SUMS.txt 中对应行比对
```

## 4. 已知限制

- 未做 Authenticode 签名，杀软可能提示；请核对哈希后再安装
- 应用内更新会校验 SHA256；空哈希将拒绝安装
- **≤1.0.5 旧客户端**只拉 `github.com` 直链取 checksums，国内超时会报「缺少 SHA256」——需手动安装一次 ≥1.0.6，之后走正文/`api.github.com` 通路即可正常更新

## 5. 若 fork 仓库

同步修改 `internal/update/update.go` 中的 `RepoOwner` / `RepoName`。
