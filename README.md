# wbwtty — 跨平台终端框架

- 最近更新：2026-10-06
- 状态：阶段 0/1/2/3 完成 → `v0.1.0-stage0`…`v0.4.0-stage3`（阶段 4 pty-win10-early 开工）
- 平台：Windows 10（含 1809 之前）/ Windows 11 / Linux / macOS

模块化终端框架：统一 PTY 抽象 + VT 解析 + GPU/GTK 渲染 + 输入编码，
运行时自动选择后端（ConPTY → 早期 Windows 10 桥接 → Unix PTY）。
每个模块都是独立 crate，可被其他项目单独依赖。

## 模块与依赖方向

```
                +-------------+     +--------------+
                |  vt-parser  |     |  term-input  |   ← 纯逻辑，零平台依赖
                +------+------+     +------+-------+
                       ^                   ^
                       | 屏幕状态           | VT 字节
        +--------------+-------------------+-------------+
        |                                                 |
+-------+-------+                               +---------+---------+
| term-render-gtk|                              |     term-app      |  ← 示例组装
| term-render-qt |  (可选)                      +---------+---------+
+-------+-------+                                        |
        ^                                                | 组合使用
        | 屏幕状态                                        v
+-------+---------+     +--------------+     +-----------+-----------+
|    pty-core     |<----|  pty-unix    |     |   pty-conpty          |
| (trait / C ABI) |<----|  pty-win10-early | | (动态加载 ConPTY)      |
+-----------------+     +--------------+     +-----------------------+
```

依赖严格单向：`vt-parser`/`term-input`/`pty-core` 无平台依赖；
后端 crate 只依赖 `pty-core`；渲染器只消费 `vt-parser` 状态；
**任何模块不得依赖 `term-app` 或其他无关模块**。

| 模块 | 职责 | 独立使用场景 |
|---|---|---|
| `crates/vt-parser` | VT 序列解析、字符网格/颜色/光标/滚动缓冲/damage | 任何终端模拟器 |
| `crates/pty-core` | PTY 抽象（spawn/read/write/resize/send_signal/close）+ C ABI | 自行实现后端的项目 |
| `crates/pty-unix` | forkpty/openpty 后端（Linux/macOS） | Unix PTY 需求 |
| `crates/pty-conpty` | ConPTY 后端（Win10 1809+，全部动态加载） | Windows 现代系统 |
| `crates/pty-win10-early` | 1809 前桥接：控制台 API 直驱 / Cygwin PTY / WinPTY 回退 | 老 Windows 终端项目 |
| `crates/term-input` | 键鼠事件 → VT 序列（纯函数） | 任何终端前端 |
| `crates/term-render-gtk` | GTK3 + Cairo/OpenGL 渲染 | GTK 终端项目 |
| `crates/term-render-qt` | Qt 渲染（可选，backlog） | Qt 终端项目 |
| `apps/term-app` | 主程序示例（仅演示组装方式） | 参考实现 |

## 组合方式（第三方项目如何引用）

只依赖需要的模块，不必引入整个终端：

```toml
# 例：只要 VT 解析 + Unix PTY
[dependencies]
vt-parser  = { path = ".../crates/vt-parser" }
pty-core   = { path = ".../crates/pty-core" }
pty-unix   = { path = ".../crates/pty-unix" }
```

```toml
# 例：Windows 上自动选择 ConPTY / 早期桥接
[dependencies]
pty-core       = { path = ".../crates/pty-core" }
pty-conpty     = { path = ".../crates/pty-conpty" }   # 1809+
pty-win10-early= { path = ".../crates/pty-win10-early" } # 1809 之前
```

```rust
// 统一入口：运行时探测可用后端，前端只与 pty-core::Pty 交互
// let mut pty = pty_core::spawn(&options)?;
```

非 Rust 项目可使用 `pty-core` 的 C ABI 头（`crates/pty-core/include/pty_core.h`，
阶段 1 提供）。

## 构建

```powershell
# 工作区整体构建
cargo build --workspace --all-targets
cargo test  --workspace

# 单独构建某个模块
cargo build -p vt-parser

# 本机会话环境（路径按仓库盘符检测，临时目录 <盘符>:\wbwtty-temp；新终端先点号引入）
. .\scripts\env.ps1

# 本机等价 CI 子集（文档检查 + 白名单 linter + 构建/测试）
powershell -File scripts/ci-local.ps1
```

平台前提：

- **Linux**：`apt-get install libgtk-3-dev pkg-config`（构建 term-render-gtk 时）
- **Windows（GTK）**：MSYS2 ucrt64（本机 `E:\吴邦玮\项目\mymsys2`，`pacman -S
  mingw-w64-ucrt-x86_64-gtk3 mingw-w64-ucrt-x86_64-pkgconf`），构建/运行前把
  `<MSYS2>\ucrt64\bin` 前置到 PATH（`scripts/env.ps1` 按候选路径自动处理）
- **Windows 后端**：1809+ 自动使用 ConPTY；更早版本使用 `pty-win10-early`
- **本机环境**：Rust 用默认 `~/.rustup`/`~/.cargo`，临时/下载在 `<仓库盘符>:\wbwtty-temp`；
  CI 的 Windows/macOS 无 GTK 开发包时排除 `term-render-gtk`（Ubuntu 权威验证）

## SDK 兼容性（阶段 0 成果）

- `sdk-headers/`：7 个 Windows 10 SDK（10240/10586/14393/15063/16299/17134/17763）
  官方头文件 include 闭包，入库供 CI 逐版本编译验证
- `docs/sdk-compat-matrix.md`：API × SDK 存在性矩阵、行为差异、各后端最低 SDK
- `docs/api-whitelist.md` + `config/api-whitelist.json`：静态链接白名单
  （62 个 7 版全绿 API；ConPTY 三函数强制动态加载）
- `docs/header-verification.md`：官方安装器交叉校验报告
- 强制检查：`python tools/check_api_whitelist.py`（本机 + CI）

## Git 工作流

- `main` 恒定可构建；开发走 `feature/<模块>`、修复走 `fix/<简述>`，CI 通过后合回
- Conventional Commits（`feat:`/`fix:`/`docs:`/`test:`/`refactor:`/`chore:`/`perf:`）
- 阶段标签：`v0.1.0-stage0` → `v0.2.0-stage1` → `v0.3.0-stage2` →
  `v0.4.0-stage3` → `v0.5.0-stage4`
- 状态看板：[`AGENT.md`](AGENT.md)；阶段计划：[`plan.md`](plan.md)

## 文档索引

- [`AGENT.md`](AGENT.md) — 当前状态、分支/提交、待办
- [`plan.md`](plan.md) — 阶段计划、Git 里程碑、验收标准
- [`docs/sdk-compat-matrix.md`](docs/sdk-compat-matrix.md) — SDK 兼容矩阵
- [`docs/api-whitelist.md`](docs/api-whitelist.md) — API 白名单
- [`docs/header-verification.md`](docs/header-verification.md) — 头文件校验
- [`docs/architecture.md`](docs/architecture.md) — 架构说明
- 各模块 `README.md` — API/示例/构建
