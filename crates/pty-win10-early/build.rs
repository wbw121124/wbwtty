//! 构建 conhook.dll（第二刀注入的子进程钩子，`csrc/conhook.c` + `early_proto.c`）。
//!
//! - MSVC：通过 vswhere 定位 VS 安装 → 调用 vcvarsall.bat 设置环境 → `cl /LD /O2 /MD`
//!   `/MD` 动态链接 ucrt.dll（自带完整 vsnprintf/snprintf）。
//! - GNU（msys2）：`gcc -shared -static-libgcc`；
//! - 任一路径失败 → 不设 `WBWTTY_EARLY_HOOK_DLL` → `inject::hook_dll_path()`
//!   返回 `None` → 注入禁用，spawn 自动回退第一刀轮询（降级不报错）。
//! 非 Windows 目标直接跳过。CI 的 sdk-compile-matrix 另行用 7 版 SDK
//! 对 `csrc/*.c` 做 `cl /c` 兼容编译（本脚本只负责产真实 DLL）。

use std::env;
use std::path::{Path, PathBuf};
use std::process::Command;

fn main() {
    println!("cargo:rerun-if-env-changed=WBWTTY_SKIP_HOOK_DLL");
    println!("cargo:rerun-if-changed=csrc/conhook.c");
    println!("cargo:rerun-if-changed=csrc/early_proto.c");
    println!("cargo:rerun-if-changed=csrc/early_proto.h");
    if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    if env::var("WBWTTY_SKIP_HOOK_DLL").is_ok() {
        return;
    }
    let out = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR")).join("conhook.dll");
    let msvc = matches!(
        env::var("CARGO_CFG_TARGET_ENV").as_deref(),
        Ok("msvc")
    );
    let built = if msvc {
        build_msvc(&out)
    } else {
        build_gnu(&out)
    };
    if built && out.is_file() {
        println!(
            "cargo:rustc-env=WBWTTY_EARLY_HOOK_DLL={}",
            out.display()
        );
    }
}

/// 用 vswhere 找 VS 安装，调 vcvarsall.bat 设置 INCLUDE/LIB/PATH，再 invoke cl。
fn build_msvc(out: &PathBuf) -> bool {
    let vs_path = match find_vs_install() {
        Some(p) => p,
        None => {
            warning("conhook: vswhere found no VS install; injection disabled");
            return false;
        }
    };
    let vcvars = vs_path.join("VC\\Auxiliary\\Build\\vcvars64.bat");
    if !vcvars.exists() {
        warning(&format!(
            "conhook: vcvars64.bat not found at {}; injection disabled",
            vcvars.display()
        ));
        return false;
    }
    // 先跑 vcvarsall 把 env 设好，再从 PATH 拿 cl
    let vcvars_out = match Command::new("cmd")
        .args(["/C", vcvars.to_string_lossy().as_ref()])
        .output()
    {
        Ok(o) if o.status.success() => o,
        result => {
            match result {
                Ok(o) => {
                    warning(&format!(
                        "conhook: vcvars64.bat exited {}; injection disabled",
                        o.status
                    ));
                    emit_output("vcvars", "stderr", &o.stderr);
                }
                Err(e) => {
                    warning(&format!(
                        "conhook: vcvars64.bat failed to start ({e}); injection disabled"
                    ));
                }
            }
            return false;
        }
    };
    // 解析 vcvars 输出的 SET 变量，合并到当前环境
    let mut cmd = Command::new("cl");
    for line in String::from_utf8_lossy(&vcvars_out.stdout).lines() {
        if let Some((k, v)) = line.split_once('=') {
            if k.starts_with("SET ") {
                let k = &k["SET ".len()..];
                cmd.env(k, v);
            }
        }
    }
    // 同时确保 PATH 包含 cl.exe 所在目录
    cmd.args([
        "/nologo",
        "/LD",
        "/O2",
        "/MD",
    ]);
    cmd.arg(format!("/Fo{}", out.parent().unwrap().display()));
    cmd.arg(format!("/Fe{}", out.display()));
    cmd.arg("csrc/conhook.c");
    cmd.arg("csrc/early_proto.c");
    run(cmd, "cl")
}

/// 用 vswhere 找最新 VS 安装路径（含 x86.x64 工具链）。
/// 尝试多个常见路径以兼容不同 VS 安装方式（Community/Enterprise/BuildTools）。
fn find_vs_install() -> Option<PathBuf> {
    let candidates = [
        r"C:\Program Files (x86)\Microsoft Visual Studio\Installer\vswhere.exe",
        r"C:\Program Files\Microsoft Visual Studio\Installer\vswhere.exe",
    ];
    for vswhere in &candidates {
        if !Path::new(vswhere).exists() {
            continue;
        }
        let out = Command::new(vswhere)
            .args([
                "-latest",
                "-products", "*",
                "-requires", "Microsoft.VisualStudio.Component.VC.Tools.x86.x64",
                "-property", "installationPath",
                "- sortBy", "installedOnDate descending",
            ])
            .output();
        match out {
            Ok(o) if o.status.success() => {
                let path = String::from_utf8_lossy(&o.stdout).trim().to_string();
                if !path.is_empty() && Path::new(&path).exists() {
                    let vcvars = Path::new(&path).join("VC\\Auxiliary\\Build\\vcvars64.bat");
                    if vcvars.exists() {
                        return Some(PathBuf::from(path));
                    }
                }
            }
            _ => continue,
        }
    }
    None
}

fn build_gnu(out: &PathBuf) -> bool {
    let tool = match cc::Build::new().try_get_compiler() {
        Ok(t) => t,
        Err(e) => {
            warning(&format!(
                "conhook: no gcc ({e}); injection disabled"
            ));
            return false;
        }
    };
    let mut cmd: Command = tool.to_command();
    cmd.args(["-shared", "-static-libgcc", "-O2", "-o"]);
    cmd.arg(out);
    cmd.arg("csrc/conhook.c");
    cmd.arg("csrc/early_proto.c");
    run(cmd, "gcc")
}

fn run(mut cmd: Command, name: &str) -> bool {
    match cmd.output() {
        Ok(o) if o.status.success() => true,
        Ok(o) => {
            warning(&format!(
                "conhook: {name} exited with {}; injection disabled",
                o.status
            ));
            emit_output(name, "stdout", &o.stdout);
            emit_output(name, "stderr", &o.stderr);
            false
        }
        Err(e) => {
            warning(&format!(
                "conhook: {name} failed to start ({e}); injection disabled"
            ));
            false
        }
    }
}

fn emit_output(name: &str, label: &str, data: &[u8]) {
    let text = String::from_utf8_lossy(data);
    for line in text.lines() {
        let trimmed = line.trim_end();
        if trimmed.is_empty() {
            continue;
        }
        warning(&format!("conhook: {name} ({label}): {trimmed}"));
    }
}

fn warning(msg: &str) {
    println!("cargo:warning={msg}");
}
