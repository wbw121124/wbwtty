# pty-unix

POSIX 伪终端后端（Linux/macOS）；Windows 下为可编译空壳。最近更新：2026-10-03。

## 实现要点

| 环节 | 做法 |
| --- | --- |
| 分配 PTY 对 | `posix_openpt` → `grantpt` → `unlockpt` → `open(ptsname)` |
| 建立会话 | `fork`；子进程 `setsid()` + `ioctl(TIOCSCTTY)` + `dup2` 到 0/1/2 |
| 启动 | `execvp`（PATH 搜索）、`chdir(cwd)`、`setenv` 应用 `SpawnOptions` |
| 尺寸 | 主端 `ioctl(TIOCSWINSZ)`（初始尺寸在 open 时即设置） |
| 信号 | `kill(-child, sig)`（进程组），失败回退 `kill(child, sig)`；`Signal` → SIGINT/SIGTERM/SIGQUIT/SIGHUP/SIGKILL |
| EOF | 主端读返回 `EIO`（从端全关）→ `Ok(0)` |
| 回收 | `try_wait` = `waitpid(WNOHANG)`；退出码 `WEXITSTATUS` 或 `128+signal` |
| 关闭 | `close()` 幂等：发 `SIGHUP` 给子进程组 + 关主端；`Drop` 尽力收割 |

注册：`pty_unix::register()` → priority **10**（见 `pty-core` README 的选择规则）。
`is_available()` = `/dev/ptmx` 或 `/dev/pts` 存在。

非 unix 平台：`is_available()` → false、`register()` 空操作（占位，供统一调用方编译）。

## 测试

`cargo test -p pty-unix`（仅 unix 编译执行）：横幅/EOF/收割、cwd+env+写回显、
resize+TERM 信号、exec 失败 = 退出码 127。Linux 上以
`cargo check -p pty-unix --target x86_64-unknown-linux-gnu` 做无链接交叉检查。
