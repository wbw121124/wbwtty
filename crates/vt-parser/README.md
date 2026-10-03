# vt-parser

- 最近更新：2026-10-03（阶段 1）
- 状态：可用；零平台依赖，任何目标可构建测试

VT 序列解析器 + 终端屏幕状态（字符网格/颜色/光标/滚动缓冲/damage）。
是本框架的“状态层”：上游是任意 PTY/管道字节流，下游是渲染器与输入编码器。

## 职责

- 增量 UTF-8 解码（跨 `feed` 调用容忍任意字节切分）
- 解析 ESC/CSI/OSC/DCS（含 `;` 与 `:` 子参数两种 SGR 写法）
- 屏幕状态：网格、宽字符/组合字符、滚动区、备用屏、回滚缓冲、行级 damage
- 模式跟踪（供 term-input 使用）：鼠标 1000/1002/1003/1006、bracketed paste、
  focus reporting、DECCKM/DECKPAM、插入模式、原点模式、自动换行
- 查询应答：DA1/DA2、DSR 5/6（结果经 `take_replies()` 输出）

## API

```rust
use vt_parser::{Terminal, TerminalOptions, Color, Attrs};

let mut t = Terminal::with_options(80, 24, TerminalOptions { scrollback_limit: 10_000 });
t.feed(b"\x1b[1;32mREADY\x1b[0m\r\n");

let view = t.get_screen();          // 快照：cells + cursor + damage 行
assert_eq!(view.line_text(0), "READY");
assert_eq!(view.cell(0, 0).unwrap().fg, Color::Indexed(2));

t.clear_damage();                   // 渲染完成后清除 damage
t.resize(120, 40);                  // 窗口尺寸变化
let _replies = t.take_replies();    // DA/DSR 应答（写回 PTY）
```

关键类型：

| 类型 | 说明 |
|---|---|
| `Terminal` | 顶层入口：`feed` / `get_screen` / `resize` / 模式 getter |
| `ScreenView` | 快照：`cells`（行优先）、`cursor`、`damage`、`line_text(y)` |
| `Cell` | `ch` + `combining` + `fg`/`bg`/`attrs`；宽字符续格 `is_continuation()` |
| `Color` | `Default` / `Indexed(u8)` / `Rgb(u8,u8,u8)` 真彩色 |
| `MouseMode` / `MouseEncoding` | 鼠标上报模式与 SGR 编码标志 |

已支持特性（对应规格阶段 1 清单）：SGR 真彩色、光标移动/擦除/滚动区/备用屏、
鼠标模式 1000/1002/1003/1006、bracketed paste、宽字符、滚动缓冲、行级 damage、
REP/ICH/DCH/ECH/IL/DL/SU/SD、DECSTBM、DECALN、RIS/软复位、LNM、DECCKM、DECKPAM。

未支持（backlog）：字符集切换（G0/G1）、DCS/DECRQSS 应答、ISO-2022 序列。

## 构建与测试

```powershell
cargo build -p vt-parser
cargo test  -p vt-parser     # 单元 + 集成（tests/vtseq.rs）
```

无外部依赖（`[dependencies]` 为空）。
