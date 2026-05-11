extern crate winres;
fn main() {
    #[cfg(windows)]
    winres::WindowsResource::new()
        .set_icon("icons/icon.ico")
        .compile()
        .unwrap();
}
