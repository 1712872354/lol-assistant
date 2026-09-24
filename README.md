# LOL 助手（lol-assistant）

轻量级英雄联盟助手：历史战绩 + 对局信息。桌面端基于 **Tauri 2（Rust）+ React 19 + TypeScript**。

> **仅支持 Windows**（依赖 Win32 进程命令行读取、注册表、NSIS 安装器）。

## 功能

- **历史战绩**：召唤师查询 / 战绩列表与明细 / 段位展示（SGP 优先，LCU 兜底）
- **对局信息**：房间 / 选人 / 对局中双方花名册、段位、近况（Live Client Data）
- **自动更新**：minisign 签名校验 + NSIS 安装 / 便携版热替换

## 目录结构

```
frontend/           React + TS 前端（Vite 6 / Tailwind 4 / TanStack Query / Zustand）
  src/lib/          公共层：类型契约、AssetImg、RatioBar、tone/rank/summoner 工具
  src/features/     按业务域分：history / gameinfo / settings
  src/stores/       Zustand store（app / history / gameinfo / update）
src-tauri/          Rust 后端（Tauri 2）
  src/lcu/          LCU 连接层：lockfile / cmdline / HTTP / WS / 监控状态机
  src/parser/       纯函数解析：LCU / SGP 战绩、队列表
  src/service/      业务聚合：history / gameinfo + 缓存与 single-flight 门闩
  src/sgp/          腾讯 SGP 云端接口（零 LCU 依赖，凭据注入）
  src/liveclient/   Live Client Data（:2999）
  src/error.rs      统一错误类型 AppError（可按变体匹配，命令边界转用户文案）
docs/               发布说明
```

## 构建

前置：Node 22+ / pnpm 9+ / Rust 1.78+（MSVC）/ NSIS（打包用）。

```powershell
pnpm install                    # 根（tauri CLI）
pnpm --dir frontend install     # 前端依赖
pnpm --dir frontend dev         # 前端开发（配合 pnpm tauri dev）
pnpm tauri build                # 发布构建（NSIS + updater 产物）
```

便携版：构建产物旁放置 `portable.flag` 即进入便携模式（更新走热替换 helper）。

## 测试与检查

```powershell
# Rust（单元测试 + lint + 格式）
cd src-tauri
cargo test
cargo clippy --all-targets -- -D warnings
cargo fmt --all -- --check

# 前端（单测 + 类型 + 构建）
pnpm --dir frontend test
pnpm --dir frontend typecheck
pnpm --dir frontend build
```

CI（`.github/workflows/ci.yml`）对以上全量把关，另跑 `tauri build --debug --no-bundle` 冒烟。

## 发布

见 `docs/RELEASE.md`。要点：GitHub Release + `latest.json`（updater 元数据）+ minisign 签名；
更新端点只信任 GitHub 官方源（勿加第三方镜像）。

## 配置

配置文件 `%APPDATA%/lol-assistant/config.json`（主题 / 关闭到托盘 / 客户端路径 /
战绩数量 pageSize / SGP 开关）。
