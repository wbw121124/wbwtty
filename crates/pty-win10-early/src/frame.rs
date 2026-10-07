//! 命名管道帧编解码（`docs/ipc-signal-protocol.md` §3/§4；纯逻辑，无 Win32 调用）。
//!
//! 布局：`[len u32 BE][type u8][payload (len-1) B]`，`len = 1 + payload_len`
//! （含 type 字节）；`len` 上限 1 MiB，超限即断管（协议错误）。
//! 未知 type 返回 [`Frame::Unknown`]（调用方丢弃，不断管——向前兼容）。

/// 帧内 `len` 字段上限（含 type 字节）。
pub const MAX_FRAME: usize = 1 * 1024 * 1024;
/// 帧头长度（len 4B + type 1B）
#[allow(dead_code)] // imp 第三刀接线使用
pub const HDR_LEN: usize = 5;
/// 协议版本（`HELLO.proto_ver`）
pub const PROTO_VER: u16 = 1;

pub const T_HELLO: u8 = 0x00;
pub const T_VT_DATA: u8 = 0x01;
pub const T_RESIZE: u8 = 0x02;
pub const T_SIG_INT: u8 = 0x03;
pub const T_SIG_TERM: u8 = 0x04;
pub const T_PING: u8 = 0x05;
pub const T_PONG: u8 = 0x06;
pub const T_EXIT: u8 = 0x07;
pub const T_ERR: u8 = 0x08;

/// 致命协议错误：接收方断管（`feed` 返回 `Err` 后调用方应停止解析）。
#[derive(Debug, PartialEq, Eq)]
pub enum ProtoError {
    /// `len == 0` 或 `len > MAX_FRAME`
    BadLen(u32),
    /// 已知 type 的 payload 长度不足
    Malformed { typ: u8, len: usize },
    /// HELLO 版本不符（宿主拒绝连接）
    VersionMismatch(u16),
}

impl std::fmt::Display for ProtoError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProtoError::BadLen(l) => write!(f, "frame len {l} out of range"),
            ProtoError::Malformed { typ, len } => {
                write!(f, "malformed frame type={typ:#04x} payload_len={len}")
            }
            ProtoError::VersionMismatch(v) => write!(f, "proto version {v} != {PROTO_VER}"),
        }
    }
}

impl std::error::Error for ProtoError {}

/// 解析完成的一帧。
#[derive(Debug, PartialEq, Eq)]
pub enum Frame {
    /// 连接后首帧：`proto_ver u16 LE` + `flags u16 LE`
    Hello { ver: u16, flags: u16 },
    /// 屏幕 diff 合成的 VT 字节（UTF-8）
    VtData(Vec<u8>),
    /// 宿主已改控制台尺寸：`cols u16 LE` + `rows u16 LE`
    Resize { cols: u16, rows: u16 },
    SigInt,
    SigTerm,
    Ping,
    Pong,
    /// child 退出，此后 conhook 发完残留帧再关管
    Exit { code: i32 },
    /// 非致命错误上报（UTF-8）
    Err(String),
    /// 未知 type：丢弃并可回 `ERR`（协议 §4 向前兼容）
    Unknown { typ: u8, payload: Vec<u8> },
}

/// 编一帧：`type + payload` → 线格式。payload 超限返回 `Err(BadLen)`。
pub fn encode(typ: u8, payload: &[u8]) -> Result<Vec<u8>, ProtoError> {
    let len = 1 + payload.len();
    if len == 0 || len > MAX_FRAME {
        return Err(ProtoError::BadLen(len as u32));
    }
    let mut out = Vec::with_capacity(4 + len);
    out.extend_from_slice(&(len as u32).to_be_bytes());
    out.push(typ);
    out.extend_from_slice(payload);
    Ok(out)
}

/// `HELLO` payload（`ver u16 LE` + `flags u16 LE`）。
#[allow(dead_code)] // imp 第三刀接线使用
pub fn hello_payload(flags: u16) -> [u8; 4] {
    let mut b = [0u8; 4];
    b[0..2].copy_from_slice(&PROTO_VER.to_le_bytes());
    b[2..4].copy_from_slice(&flags.to_le_bytes());
    b
}

/// `RESIZE` payload（`cols u16 LE` + `rows u16 LE`）。
pub fn resize_payload(cols: u16, rows: u16) -> [u8; 4] {
    let mut b = [0u8; 4];
    b[0..2].copy_from_slice(&cols.to_le_bytes());
    b[2..4].copy_from_slice(&rows.to_le_bytes());
    b
}

/// `EXIT` payload（`code i32 LE`）。
#[allow(dead_code)] // imp 第三刀接线使用
pub fn exit_payload(code: i32) -> [u8; 4] {
    code.to_le_bytes()
}

/// 流式拆帧器：喂字节（消息模式一次一帧也走这里，容忍粘包/半包）。
pub struct Parser {
    buf: Vec<u8>,
}

impl Parser {
    pub fn new() -> Self {
        Self { buf: Vec::new() }
    }

    /// 追加字节，取出所有完整帧。致命错误（坏 len / 坏已知帧）返回 `Err`
    /// 并清空缓冲（断管语义：调用方此后不得继续解析）。
    pub fn feed(&mut self, chunk: &[u8]) -> Result<Vec<Frame>, ProtoError> {
        self.buf.extend_from_slice(chunk);
        let mut out = Vec::new();
        loop {
            if self.buf.len() < 4 {
                break;
            }
            let len = u32::from_be_bytes(self.buf[0..4].try_into().unwrap()) as usize;
            if len == 0 || len > MAX_FRAME {
                self.buf.clear();
                return Err(ProtoError::BadLen(len as u32));
            }
            let total = 4 + len;
            if self.buf.len() < total {
                break;
            }
            let frame_bytes: Vec<u8> = self.buf.drain(..total).collect();
            match parse_frame(&frame_bytes[4..total]) {
                Ok(f) => out.push(f),
                Err(e) => {
                    self.buf.clear();
                    return Err(e);
                }
            }
        }
        Ok(out)
    }
}

impl Default for Parser {
    fn default() -> Self {
        Self::new()
    }
}

/// 解析 `[type][payload]`（已去掉 len 头）。
fn parse_frame(body: &[u8]) -> Result<Frame, ProtoError> {
    let typ = body[0];
    let payload = &body[1..];
    let malformed = |len: usize| ProtoError::Malformed { typ, len };
    Ok(match typ {
        T_HELLO => {
            if payload.len() < 4 {
                return Err(malformed(payload.len()));
            }
            let ver = u16::from_le_bytes([payload[0], payload[1]]);
            let flags = u16::from_le_bytes([payload[2], payload[3]]);
            if ver != PROTO_VER {
                return Err(ProtoError::VersionMismatch(ver));
            }
            Frame::Hello { ver, flags }
        }
        T_VT_DATA => Frame::VtData(payload.to_vec()),
        T_RESIZE => {
            if payload.len() < 4 {
                return Err(malformed(payload.len()));
            }
            Frame::Resize {
                cols: u16::from_le_bytes([payload[0], payload[1]]),
                rows: u16::from_le_bytes([payload[2], payload[3]]),
            }
        }
        T_SIG_INT => Frame::SigInt,
        T_SIG_TERM => Frame::SigTerm,
        T_PING => Frame::Ping,
        T_PONG => Frame::Pong,
        T_EXIT => {
            if payload.len() < 4 {
                return Err(malformed(payload.len()));
            }
            Frame::Exit {
                code: i32::from_le_bytes([payload[0], payload[1], payload[2], payload[3]]),
            }
        }
        T_ERR => Frame::Err(String::from_utf8_lossy(payload).into_owned()),
        _ => Frame::Unknown { typ, payload: payload.to_vec() },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_all_known_types() {
        let cases: Vec<(u8, Vec<u8>, Frame)> = vec![
            (T_HELLO, hello_payload(0).to_vec(), Frame::Hello { ver: 1, flags: 0 }),
            (T_VT_DATA, b"\x1b[1;1Hhi".to_vec(), Frame::VtData(b"\x1b[1;1Hhi".to_vec())),
            (T_RESIZE, resize_payload(137, 53).to_vec(), Frame::Resize { cols: 137, rows: 53 }),
            (T_SIG_INT, vec![], Frame::SigInt),
            (T_SIG_TERM, vec![], Frame::SigTerm),
            (T_PING, vec![], Frame::Ping),
            (T_PONG, vec![], Frame::Pong),
            (T_EXIT, exit_payload(-1).to_vec(), Frame::Exit { code: -1 }),
            (T_ERR, b"hook down".to_vec(), Frame::Err("hook down".into())),
        ];
        for (typ, payload, expect) in cases {
            let wire = encode(typ, &payload).expect("encode");
            let mut p = Parser::new();
            let frames = p.feed(&wire).expect("parse");
            assert_eq!(frames, vec![expect], "type {typ:#04x}");
            assert!(p.feed(&[]).unwrap().is_empty(), "缓冲应已耗尽");
        }
    }

    #[test]
    fn truncated_then_completed_arrives_once() {
        let wire = encode(T_VT_DATA, b"hello-vt").unwrap();
        let mut p = Parser::new();
        assert!(p.feed(&wire[..3]).unwrap().is_empty(), "半包不产出");
        assert!(p.feed(&wire[3..6]).unwrap().is_empty(), "半包不产出");
        let frames = p.feed(&wire[6..]).unwrap();
        assert_eq!(frames, vec![Frame::VtData(b"hello-vt".to_vec())]);
    }

    #[test]
    fn multiple_frames_in_one_chunk() {
        let a = encode(T_PING, &[]).unwrap();
        let b = encode(T_EXIT, &exit_payload(7)).unwrap();
        let mut all = a.clone();
        all.extend_from_slice(&b);
        let mut p = Parser::new();
        let frames = p.feed(&all).unwrap();
        assert_eq!(frames, vec![Frame::Ping, Frame::Exit { code: 7 }]);
    }

    #[test]
    fn oversized_len_is_fatal() {
        let mut p = Parser::new();
        let bad = ((MAX_FRAME + 1) as u32).to_be_bytes();
        assert_eq!(p.feed(&bad), Err(ProtoError::BadLen((MAX_FRAME + 1) as u32)));
        // 断管后缓冲已清：后续合法帧也不会被吞掉状态
        let ok = encode(T_PONG, &[]).unwrap();
        assert_eq!(p.feed(&ok).unwrap(), vec![Frame::Pong]);
    }

    #[test]
    fn zero_len_is_fatal() {
        let mut p = Parser::new();
        assert_eq!(p.feed(&0u32.to_be_bytes()), Err(ProtoError::BadLen(0)));
    }

    #[test]
    fn malformed_hello_and_short_frames_are_fatal() {
        let mut p = Parser::new();
        let wire = encode(T_HELLO, &[0x01]).unwrap(); // 只有 1 字节 payload
        assert!(matches!(
            p.feed(&wire),
            Err(ProtoError::Malformed { typ: T_HELLO, len: 1 })
        ));

        let mut p = Parser::new();
        let wire = encode(T_EXIT, &[1, 2]).unwrap();
        assert!(matches!(p.feed(&wire), Err(ProtoError::Malformed { typ: T_EXIT, .. })));
    }

    #[test]
    fn version_mismatch_rejected() {
        let mut bad = [0u8; 4];
        bad[0..2].copy_from_slice(&99u16.to_le_bytes());
        let wire = encode(T_HELLO, &bad).unwrap();
        let mut p = Parser::new();
        assert_eq!(p.feed(&wire), Err(ProtoError::VersionMismatch(99)));
    }

    #[test]
    fn unknown_type_yields_unknown_frame() {
        let wire = encode(0x42, b"future").unwrap();
        let mut p = Parser::new();
        let frames = p.feed(&wire).unwrap();
        assert_eq!(
            frames,
            vec![Frame::Unknown { typ: 0x42, payload: b"future".to_vec() }]
        );
    }

    #[test]
    fn encode_rejects_oversize_payload() {
        let big = vec![0u8; MAX_FRAME]; // 1 + len > MAX
        assert!(matches!(encode(T_VT_DATA, &big), Err(ProtoError::BadLen(_))));
    }
}
