use std::path::Path;

fn main() {
    if Path::new("../rosy-compiler").is_dir() {
        println!("cargo:warning=using local rosy-compiler");
    }
}
