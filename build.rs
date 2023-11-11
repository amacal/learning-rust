use std::process::Command;
use std::env;

fn main() {
    let dir = env::current_dir().unwrap();
    let path = dir.join("asm/extract.asm");
    let out_dir = env::var("OUT_DIR").unwrap();

    Command::new("nasm")
        .args(&["-f", "elf64", "-o"])
        .arg(&format!("{}/extract.o", out_dir))
        .arg(&path)
        .status()
        .expect("Failed to run NASM!");

    Command::new("ar")
        .args(&["rcs"])
        .arg(&format!("{}/libamacal.a", out_dir))
        .arg(&format!("{}/extract.o", out_dir))
        .status()
        .expect("Failed to run AR!");

    println!("cargo:rustc-link-search=native={}", out_dir);
    println!("cargo:rustc-link-arg=-l:libamacal.a");
}
