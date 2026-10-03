# AGENT.md — 项目状态与执行记录

- 最近更新：2026-10-03

## 当前 Git 状态

- 当前分支：main
- 最近提交：（见下方提交摘要，初始提交前为空）
- 已打标签：（无）
- 待合并分支：（无）

## 当前阶段

**阶段 0：SDK 兼容性调研（进行中）**。阶段 0 未完成前不进入编码阶段（规格硬约束）。

## 模块划分（计划）

| 模块 | 职责 | 状态 |
|---|---|---|
| vt-parser | VT 序列解析 + 终端状态（网格/颜色/光标/滚动缓冲/damage） | 未开始 |
| pty-core | PTY 统一抽象（trait + C ABI），无平台实现 | 未开始 |
| pty-unix | openpty/forkpty 后端（Linux/macOS） | 未开始 |
| pty-conpty | ConPTY 后端（Win10 1809+，全部动态加载） | 未开始 |
| pty-win10-early | 1809 前桥接：控制台 API 直驱 / Cygwin PTY / WinPTY 回退 | 未开始 |
| term-render-gtk | GTK3+Cairo/OpenGL 渲染 | 未开始 |
| term-render-qt | Qt 渲染（可选，backlog） | 未开始 |
| term-input | 键鼠事件 → VT 序列编码（纯函数） | 未开始 |
| term-app | 组装示例主程序 | 未开始 |

## 已完成任务

- 2026-10-03：需求确认（SDK 获取方式、CI、GTK、执行范围、Git 身份）
- 2026-10-03：git init（main）+ 仓库 local 身份 + .gitignore + plan.md + AGENT.md

## 下一步（阶段 0 待办）

1. fetch-win-headers.ps1：稀疏克隆 ralish/win-headers 7 版头文件 → D:\temp
2. prune-headers.py：include 闭包裁剪 → sdk-headers/<ver>/
3. verify-sdk-installers.ps1：官方安装器（10240/17763）静默安装交叉校验
4. gen-compat-matrix.py → docs/sdk-compat-matrix.md；api-whitelist.json/md
5. check_api_whitelist.py + CI 配置 + docs 齐套
6. 提交 `docs: add SDK compatibility matrix and API whitelist`，标签 v0.1.0-stage0
7. 安装 gh 并认证，创建远端仓库并推送

## 已知问题 / 阻塞

- 本机无 MSVC/Windows SDK/winget/7z → per-SDK 编译由 CI 承担（已决策）
- 本机无 gh CLI 且未认证 → 远端仓库创建需一次用户认证（device flow 或 GH_TOKEN）

## SDK 兼容性结论（摘要，详见 docs/sdk-compat-matrix.md）

- 结论生成中。已知事实：CreatePseudoConsole/ResizePseudoConsole/ClosePseudoConsole
  仅在 10.0.17763+ 头文件中声明（1809 起提供）；本机 build 17763 已验证三函数导出存在。

## API 白名单摘要（详见 docs/api-whitelist.md）

- 生成中。原则：白名单 = 所有早期 SDK（10240..17134）头文件中均静态声明的 API；
  其余一律 LoadLibrary+GetProcAddress 动态加载并做版本检测，禁止静态链接。

## 约定

- 提交：Conventional Commits（feat/fix/docs/test/refactor/chore/perf），单一逻辑变更
- 提交前：相关模块构建+测试通过；禁止提交编译产物/密钥/个人信息
- 标签：v0.x.0-stageN，仅打在 main 上；文档随代码同步更新
- 下载与解压一律 D:\temp（不用 C:、不用 F:）
