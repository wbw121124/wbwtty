//! 构建 conhook.dll（第二刀注入的子进程钩子，`csrc/conhook.c` + `early_proto.c`）。
//!
//! - MSVC：用 `cc` 定位 `cl`（注册表发现，dev prompt 之外也可用）→ `/LD /O2 /MT`
//!   产出静态 CRT 的自包含 DLL；
//!   CI 的 plain-bash 步骤无 vcvars env → 额外用 vswhere 找 `vcvars64.bat`，
//!   执行 `set` 抓取完整 VC env（INCLUDE/LIB/LIBPATH/PATH），避免 `cl` 因找不到
//!   Windows SDK headers/libraries 而退出 code 2（C1083/链接失败）。
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

fn build_msvc(out: &PathBuf) -> bool {
    let tool = match cc::Build::new().try_get_compiler() {
        Ok(t) => t,
        Err(e) => {
            warning(&format!(
                "conhook: no MSVC compiler ({e}); injection disabled"
            ));
            return false;
        }
    };
    // cl：/MT 须在 cc 注入的参数之后（同组选项后者生效）→ 静态 CRT，DLL 自包含
    let mut cmd: Command = tool.to_command();
    cmd.arg("/nologo");
    cmd.arg("/LD");
    cmd.arg("/O2");
    cmd.arg("/MT");
    cmd.arg(format!("/Fo{}\\", out.parent().unwrap().display()));
    cmd.arg(format!("/Fe{}", out.display()));
    cmd.arg("csrc/conhook.c");
    cmd.arg("csrc/early_proto.c");

    // CI 的 plain-bash 步骤（无 msvc-dev-cmd）往往拿不到完整 VC env；
    // vswhere 能定位已安装的 VS → 跑 vcvars64.bat → `set` 把完整 env 打印到 stdout。
    // 这里只做追加覆盖（保留 cargo/runner 注入的变量），确保 INCLUDE/LIB/LIBPATH 齐全。
    let vcvars = find_vcvars64();
    if let Some(p) = &vcvars {
        let script = format!(
            "call \"{}\" >nul 2>&1 && set",
            p.display()
        );
        if let Ok(o) = Command::new("cmd").arg("/C").arg(&script).output() {
            if o.status.success() {
                for line in String::from_utf8_lossy(&o.stdout).lines() {
                    if let Some((k, v)) = line.split_once('=') {
                        cmd.env(k, v);
                    }
                }
            }
        }
        let msg = if vcvars_env_set(&cmd) {
            "vcvars"
        } else {
            "vcvars (env not applied)"
        };
        eprintln!(
            "[early-diag] build_msvc: using {} at {:?}",
            msg, p
        );
    } else {
        eprintln!("[early-diag] build_msvc: vcvars64.bat not found; relying on cc's env");
    }

    run(cmd, "cl")
}

fn vcvars_env_set(cmd: &Command) -> bool {
    // 保守判断：命令行里不含显式的 /I，则依赖 env；若 env 中不含 INCLUDE，视为未设置
    let has_include = cmd
        .get_envs()
        .any(|(k, _)| k.eq_ignore_ascii_case("INCLUDE"));
    has_include
}

/// 用 vswhere 定位当前机器上最新的含 x86.x64 工具链的 VS 安装，再构造 vcvars64.bat 路径。
/// 兜底：试几个常见 VS2022 安装路径（Enterprise/Community/Professional/BuildTools）。
fn find_vcvars64() -> Option<PathBuf> {
    // vswhere 常见位置
    let vswhere_candidates = [
        r"C:\Program Files (x86)\Microsoft Visual Studio\Installer\vswhere.exe",
        r"C:\Program Files\Microsoft Visual Studio\Installer\vswhere.exe",
    ];
    for vswhere in &vswhere_candidates {
        if !Path::new(vswhere).exists() {
            continue;
        }
        let out = Command::new(vswhere)
            .args([
                "-latest",
                "-products",
                "*",
                "-requires",
                "Microsoft.VisualStudio.Component.VC.Tools.x86.x64",
                "-property",
                "installationPath",
            ])
            .output();
        if let Ok(o) = out {
            if o.status.success() {
                let inst = String::from_utf8_lossy(&o.stdout).trim().to_string();
                if !inst.is_empty() {
                    let p = Path::new(&inst).join("VC\\Auxiliary\\Build\\vcvars64.bat");
                    if p.exists() {
                        return Some(p);
                    }
                }
            }
        }
    }
    // 兜底：直接试常见路径
    for variant in &[
        "Enterprise",
        "Community",
        "Professional",
        "BuildTools",
    ] {
        let p = format!(
            "C:\\Program Files\\Microsoft Visual Studio\\2022\\{variant}\\VC\\Auxiliary\\Build\\vcvars64.bat"
        );
        if Path::new(&p).exists() {
            return Some(PathBuf::from(p));
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
