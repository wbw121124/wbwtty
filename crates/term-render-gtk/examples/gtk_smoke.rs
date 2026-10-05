//! GTK 链接/运行时冒烟：`gtk::init()` 成功即证明 ucrt64 GTK3 与 Rust(gnu) 链接可用。
//!
//! 运行：`cargo run -p term-render-gtk --example gtk_smoke`（PATH 需含 D:\msys\ucrt64\bin）。

fn main() {
    match gtk::init() {
        Ok(()) => {
            println!("[gtk-smoke] gtk::init OK - GTK3 linked and loadable");
        }
        Err(e) => {
            eprintln!("[gtk-smoke] gtk::init FAILED: {e}");
            std::process::exit(1);
        }
    }
}
