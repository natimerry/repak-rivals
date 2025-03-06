extern crate winres;
fn main() {
    #[cfg(target_os = "windows")]
    winres::WindowsResource::new()
        .set_icon("icons/icon.ico")
        .compile()
        .unwrap();
}