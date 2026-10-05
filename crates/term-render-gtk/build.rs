fn main() {
    // gtk-rs 0.18 未绑定 `pango_cairo_*`，本 crate 自行声明该符号；
    // 这里用 pkg-config 解析 libpangocairo-1.0 的链接参数（Linux CI 与 MSYS2 ucrt64 同路径）。
    println!("cargo:rerun-if-changed=build.rs");
    if let Err(e) = pkg_config::Config::new().probe("pangocairo") {
        panic!(
            "pkg-config 未找到 pangocairo（需要 pango + cairo 开发包）: {e}\n\
             Windows/MSYS2: pacman -S mingw-w64-ucrt-x86_64-gtk3；Linux: libgtk-3-dev"
        );
    }
}
