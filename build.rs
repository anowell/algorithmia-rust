use std::fs::File;
use std::io::Write;

fn main() {
    // Write it to version.rs
    let mut f = File::create("src/version.rs").unwrap();
    write!(
        f,
        r#"
pub static RUSTC_VERSION: &'static str = "{rustc_version}";
"#,
        rustc_version = rustc_version::version().unwrap(),
    )
    .unwrap();
}
