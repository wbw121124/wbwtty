//! 构建 conhook.dll（第二刀注入的子进程钩子，`csrc/conhook.c` + `early_proto.c`）。
//!
//! - MSVC：vswhere 定位 VS 安装 → 写辅助 bat（`cd OUT_DIR` + `call vcvars64.bat`
//!   + `cl /LD /O2 /MD`，同会话内环境不丢失，也避开 cmd 内嵌引号的转义陷阱）→
//!   `cmd /C` 执行；`/MD` 动态链接 ucrt.dll（自带完整 vsnprintf/snprintf）。
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

/// 用 vswhere 找 VS 安装，再通过辅助 bat 在同一 cmd 会话里
/// `call vcvars64.bat` 后直接 `cl`（环境只活在该 cmd 进程内，
/// 无法回传给外部进程）。
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
    let out_dir = out.parent().expect("OUT_DIR");
    // 辅助 bat 写入文件：含空格/引号的路径经 Rust Command 传给 cmd /C
    // 会被 \" 转义，cmd 不认这种转义 → 必须避开命令行内嵌引号。
    let crate_dir = env::current_dir().expect("cwd");
    let script = out_dir.join("conhook-build.bat");
    let bat = format!(
        "@echo off\r\n\
         cd /d \"{out_dir}\"\r\n\
         call \"{vcvars}\"\r\n\
         if errorlevel 1 exit /b 1\r\n\
         cl /nologo /utf-8 /LD /O2 /MD /Fe\"{dll}\" \"{s1}\" \"{s2}\" legacy_stdio_definitions.lib\r\n",
        out_dir = out_dir.display(),
        vcvars = vcvars.display(),
        dll = out.display(),
        s1 = crate_dir.join("csrc\\conhook.c").display(),
        s2 = crate_dir.join("csrc\\early_proto.c").display(),
    );
    if let Err(e) = std::fs::write(&script, bat) {
        warning(&format!(
            "conhook: cannot write {}: {e}; injection disabled",
            script.display()
        ));
        return false;
    }
    let mut cmd = Command::new("cmd");
    cmd.arg("/C").arg(&script);
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
            Ok(o) => {
                warning(&format!(
                    "conhook: vswhere exited {} ({})",
                    o.status,
                    vswhere
                ));
                emit_output("vswhere", "stdout", &o.stdout);
                emit_output("vswhere", "stderr", &o.stderr);
            }
            Err(e) => warning(&format!("conhook: vswhere failed to start ({e})")),
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
