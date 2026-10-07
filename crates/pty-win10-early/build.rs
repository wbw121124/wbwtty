//! 构建 conhook.dll（第二刀注入的子进程钩子，`csrc/conhook.c` + `early_proto.c`）。
//!
//! - MSVC：用 `cc` 定位 `cl`（注册表发现，dev prompt 之外也可用）→ `/LD /O2 /MT`
//!   产出静态 CRT 的自包含 DLL；
//! - GNU（msys2）：`gcc -shared -static-libgcc`；
//! - 任一路径失败 → 不设 `WBWTTY_EARLY_HOOK_DLL` → `inject::hook_dll_path()`
//!   返回 `None` → 注入禁用，spawn 自动回退第一刀轮询（降级不报错）。
//! 非 Windows 目标直接跳过。CI 的 sdk-compile-matrix 另行用 7 版 SDK
//! 对 `csrc/*.c` 做 `cl /c` 兼容编译（本脚本只负责产真实 DLL）。

use std::env;
use std::path::PathBuf;
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
    let msvc = matches!(env::var("CARGO_CFG_TARGET_ENV").as_deref(), Ok("msvc"));
    let built = if msvc { build_msvc(&out) } else { build_gnu(&out) };
    if built && out.is_file() {
        println!("cargo:rustc-env=WBWTTY_EARLY_HOOK_DLL={}", out.display());
    }
}

fn build_msvc(out: &PathBuf) -> bool {
    let tool = match cc::Build::new().try_get_compiler() {
        Ok(t) => t,
        Err(e) => {
            println!("cargo:warning=conhook: no MSVC compiler ({e}); injection disabled");
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
    run(cmd, "cl")
}

fn build_gnu(out: &PathBuf) -> bool {
    let tool = match cc::Build::new().try_get_compiler() {
        Ok(t) => t,
        Err(e) => {
            println!("cargo:warning=conhook: no gcc ({e}); injection disabled");
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
    match cmd.status() {
        Ok(s) if s.success() => true,
        Ok(s) => {
            println!("cargo:warning=conhook: {name} exited with {s}; injection disabled");
            false
        }
        Err(e) => {
            println!("cargo:warning=conhook: {name} failed to start ({e}); injection disabled");
            false
        }
    }
}
