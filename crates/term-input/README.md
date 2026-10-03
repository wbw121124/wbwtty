# term-input

键盘/鼠标/滚轮/paste 事件 → VT 序列编码（纯逻辑，零平台依赖）。最近更新：2026-10-03。

## 职责

| 能力 | API |
| --- | --- |
| 平台无关事件 | `InputEvent::{Key, Mouse, Paste}`、`KeyCode`/`Modifiers`/`MouseEvent` |
| 编码状态机 | `Encoder`：DECCKM、DECKPAM、鼠标模式、坐标编码、bracketed paste |
| 编码入口 | `encode(&InputEvent) -> Option<Vec<u8>>`、`encode_paste(&str) -> Vec<u8>` |

平台适配层（GTK/Qt/Win32）负责把原生事件翻译成 `InputEvent`（大小写归一、1 起始坐标、
ISOLeftTab → `Tab`+SHIFT），**不**依赖本 crate 的任何平台代码。

## 编码约定（xterm）

- 修饰参数 = `1 + Shift(1) + Alt(2) + Ctrl(4) + Meta(8)`，如 Ctrl+Shift+↑ = `CSI 1;6A`
- 光标/Home/End：无修饰 + DECCKM → `SS3 X`，否则 `CSI X`；带修饰一律 `CSI 1;mX`
- Insert/Delete/PgUp/PgDn、F5..F12 → `CSI n~`（带修饰 `CSI n;m~`）
- F1..F4 → `SS3 P/Q/R/S`（带修饰 `CSI 1;mP`…）
- 字符：Ctrl → C0（`c&0x1f`；空格→NUL、`?`→DEL），Alt → `ESC` 前缀；Shift+Tab → `CSI Z`
- 小键盘：DECKPAM（应用键盘）→ `SS3 p..y/n/M/k/m/l/o/j`，否则常规字符
- 鼠标（1 起始坐标）：
  - legacy（无 1006）：`CSI M (32+code) (32+col) (32+row)`，坐标封顶 223
  - SGR（DECSET 1006）：`CSI < code ; col ; row M/m`（释放 = `m`）
  - 按钮码 + motion(32) + Shift(4)/Alt(8)/Ctrl(16)；滚轮 = 64/65
- 门控：按键/滚轮需任意鼠标模式；释放需 ≥1000；拖拽需 ≥1002；无键移动需 1003
- bracketed paste（2004）：`CSI 200~ … CSI 201~`，关闭时原样输出

## 模式回灌

`Encoder` 不解析字节流——终端侧（vt-parser）检出 DECSET/DECRST 后调用
`set_cursor_keys_app` / `set_keypad_app` / `set_mouse_mode` / `set_encoding` /
`set_bracketed_paste`，保证输入编码始终匹配当前终端状态。

## 测试

`cargo test -p term-input`：11 单元 + 6 集成（修饰键矩阵、鼠标门控矩阵、
应用键盘/光标模式、粘贴包裹、事件流顺序、全键覆盖）。
