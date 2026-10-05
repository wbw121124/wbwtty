//! 命令行与环境块构造（纯 Rust，不调用 Win32，便于单测）。

use std::ffi::{OsStr, OsString};
use std::os::windows::ffi::OsStrExt;

use pty_core::SpawnOptions;

/// 按 Windows 解析语义（`CommandLineToArgvW` / CRT 参数规则）给单个参数加引号。
///
/// 规则：含空白、引号或为空时加双引号；引号前的反斜杠翻倍，
/// 字符串结尾的反斜杠在闭合引号前也要翻倍（否则会被引号吃掉）。
pub fn quote(arg: &OsStr) -> Vec<u16> {
    let wide: Vec<u16> = arg.encode_wide().collect();
    let needs_quotes = wide.is_empty()
        || wide
            .iter()
            .any(|&c| c == u16::from(b' ') || c == u16::from(b'\t') || c == u16::from(b'"'));
    if !needs_quotes {
        return wide;
    }

    let mut out = Vec::with_capacity(wide.len() + 2);
    out.push(u16::from(b'"'));
    let mut backslashes = 0usize;
    for &c in &wide {
        if c == u16::from(b'\\') {
            backslashes += 1;
            continue;
        }
        if c == u16::from(b'"') {
            out.resize(out.len() + backslashes * 2 + 1, u16::from(b'\\'));
            out.push(c);
            backslashes = 0;
            continue;
        }
        out.resize(out.len() + backslashes, u16::from(b'\\'));
        backslashes = 0;
        out.push(c);
    }
    out.resize(out.len() + backslashes * 2, u16::from(b'\\'));
    out.push(u16::from(b'"'));
    out
}

/// 完整命令行（UTF-16，末尾 NUL）。
///
/// 交给 `CreateProcessW` 时 `lpApplicationName` 传 NULL，
/// 由它按命令行第一段做 PATH 搜索（与 `pty-unix` 的 `execvp` 语义对齐）。
pub fn build_command_line(opts: &SpawnOptions) -> Vec<u16> {
    let mut out: Vec<u16> = Vec::new();
    out.extend(quote(opts.program.as_os_str()));
    for a in &opts.args {
        out.push(u16::from(b' '));
        out.extend(quote(a));
    }
    out.push(0);
    out
}

/// 环境块（UTF-16，`K=V\0…\0` 双 NUL 结尾）。
///
/// - `overrides` 为空 → 返回空块，调用方传 NULL 全量继承父进程环境
/// - 否则取父进程环境并按键覆盖（大小写不敏感），按小写键排序后编码
pub fn build_environment(overrides: &[(OsString, OsString)]) -> Vec<u16> {
    if overrides.is_empty() {
        return Vec::new();
    }

    let mut entries: Vec<(OsString, OsString)> = std::env::vars_os().collect();
    for (k, v) in overrides {
        let folded = fold_key(k);
        match entries.iter_mut().find(|(ek, _)| fold_key(ek) == folded) {
            Some(slot) => slot.1 = v.clone(),
            None => entries.push((k.clone(), v.clone())),
        }
    }
    entries.sort_by(|a, b| fold_key(&a.0).cmp(&fold_key(&b.0)));

    let mut block: Vec<u16> = Vec::new();
    for (k, v) in entries {
        block.extend(k.encode_wide());
        block.push(u16::from(b'='));
        block.extend(v.encode_wide());
        block.push(0);
    }
    block.push(0);
    block
}

fn fold_key(k: &OsStr) -> String {
    k.to_string_lossy().to_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text(w: &[u16]) -> String {
        String::from_utf16_lossy(w)
    }

    fn split_block(block: &[u16]) -> Vec<String> {
        let mut out = Vec::new();
        let mut cur: Vec<u16> = Vec::new();
        // 去掉最后一个 NUL（块终止符），其余 NUL 是条目分隔
        for &c in &block[..block.len().saturating_sub(1)] {
            if c == 0 {
                out.push(String::from_utf16_lossy(&cur));
                cur.clear();
            } else {
                cur.push(c);
            }
        }
        out
    }

    #[test]
    fn quote_leaves_plain_args_alone() {
        assert_eq!(text(&quote(OsStr::new("cmd.exe"))), "cmd.exe");
        assert_eq!(text(&quote(OsStr::new("/c"))), "/c");
    }

    #[test]
    fn quote_wraps_empty_and_whitespace() {
        assert_eq!(text(&quote(OsStr::new(""))), "\"\"");
        assert_eq!(text(&quote(OsStr::new("a b"))), "\"a b\"");
        assert_eq!(text(&quote(OsStr::new("a\tb"))), "\"a\tb\"");
    }

    #[test]
    fn quote_escapes_inner_and_trailing_quotes() {
        assert_eq!(text(&quote(OsStr::new("say \"hi\""))), "\"say \\\"hi\\\"\"");
        // 结尾反斜杠在闭合引号前翻倍
        assert_eq!(
            text(&quote(OsStr::new("C:\\dir with space\\"))),
            "\"C:\\dir with space\\\\\""
        );
    }

    #[test]
    fn command_line_is_program_then_quoted_args_with_nul() {
        let opts = SpawnOptions::new("cmd.exe").args(["/c", "echo hi there"]);
        let line = build_command_line(&opts);
        assert_eq!(line.last(), Some(&0));
        assert_eq!(text(&line[..line.len() - 1]), "cmd.exe /c \"echo hi there\"");
    }

    #[test]
    fn environment_block_empty_overrides_means_inherit() {
        assert!(build_environment(&[]).is_empty());
    }

    #[test]
    fn environment_block_is_sorted_double_nul_and_contains_override() {
        let block = build_environment(&[("WT_PTY_TEST".into(), "42".into())]);
        assert!(block.len() > 2, "block should carry inherited variables");
        assert_eq!(block.last(), Some(&0));
        assert_eq!(block[block.len() - 2], 0);

        let entries = split_block(&block);
        let keys: Vec<String> = entries
            .iter()
            .map(|e| e.split('=').next().unwrap_or("").to_lowercase())
            .collect();
        let mut sorted = keys.clone();
        sorted.sort();
        assert_eq!(keys, sorted, "环境块必须按小写键排序");

        let hits: Vec<String> = entries
            .into_iter()
            .filter(|e| e.to_lowercase().starts_with("wt_pty_test="))
            .collect();
        assert_eq!(hits, ["WT_PTY_TEST=42"]);
    }

    #[test]
    fn environment_block_overrides_existing_key_case_insensitively() {
        if std::env::var_os("PATH").is_none() {
            return;
        }
        let block = build_environment(&[("path".into(), "sentinel-value".into())]);
        let hits: Vec<String> = split_block(&block)
            .into_iter()
            .filter(|e| e.to_lowercase().starts_with("path="))
            .collect();
        assert_eq!(hits.len(), 1, "PATH 只应出现一次：{hits:?}");
        assert_eq!(hits[0].split('=').nth(1), Some("sentinel-value"));
    }
}
