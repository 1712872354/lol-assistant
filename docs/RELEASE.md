# 发版步骤（GitHub）

仓库需 **Public**（国内用户才能走镜像下载 Release 资产）。

## 1. 首次上传

```powershell
cd lol-assistant
git init
git add .
git commit -m "chore: 初始提交 LOL助手 v0.1.0 功能集"
git branch -M main
git remote add origin https://github.com/1712872354/lol-assistant.git
git push -u origin main
```

> 若 GitHub 用户名/仓库名不同，同步改 `internal/update/update.go` 里的 `RepoOwner` / `RepoName`。

## 2. 发布 v0.1.0

Tag 会触发 `.github/workflows/release.yml`，自动打包并创建 Release。

```powershell
git tag -a v0.1.0 -m "v0.1.0 首个公开版本"
git push origin v0.1.0
```

## 3. Release 标题 / 描述模板

**标题**：`v0.1.0`

**描述**（可直接粘贴，或改用 `CHANGELOG.md` 对应小节）：

```markdown
## 安装

| 包 | 说明 |
|----|------|
| `LOL助手-Setup-0.1.0.exe` | NSIS 安装包（推荐） |
| `LOL助手-Portable-0.1.0.zip` | 绿色版，解压即用 |
| `SHA256SUMS.txt` | 校验哈希 |

Windows x64 · 需 WebView2 运行时

## 本版亮点

- 战绩 / 对局双页：10 人明细、近况与威胁对比
- 系统托盘：关闭到托盘、菜单唤回
- 应用内更新：检查 → 下载并安装（自动校验 SHA256）

## 校验

```powershell
Get-FileHash .\LOL助手-Setup-0.1.0.exe -Algorithm SHA256
# 与 SHA256SUMS.txt 中对应行比对
```

## 已知限制

- 未签名，杀软可能提示；请核对哈希后再安装
```

## 4. 之后每个版本

1. 更新 `CHANGELOG.md`（新 `## [vX.Y.Z]` 小节）  
2. `git commit` 后打 Tag：`git tag -a vX.Y.Z -m "vX.Y.Z"`  
3. `git push origin main --tags`  
4. 在 Release 页贴 `CHANGELOG.md` 对应段落（Actions 已挂上安装包与哈希）

## 5. 客户端侧对应关系

- 当前版本：`ldflags -X main.version=vX.Y.Z`（由 `release.yml` 注入）
- 检查更新：GitHub `releases/latest`（含国内镜像）
- 资产识别：文件名含 `Setup`/`installer` 的 exe；`SHA256SUMS.txt` 内匹配安装包哈希
