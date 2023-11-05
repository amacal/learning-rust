use std::process::Command;
use std::env;

fn main() {
    let dir = env::current_dir().unwrap();
    let asm_file = dir.join("src/extract.asm");
    let out_dir = env::var("OUT_DIR").unwrap();

    Command::new("nasm")
        .args(&["-f", "elf64", "-o"])
        .arg(&format!("{}/extract.o", out_dir))
        .arg(asm_file)
        .status()
        .expect("Failed to run nasm");

    Command::new("ar")
        .args(&["rcs"])
        .arg(&format!("{}/libamacal.a", out_dir))
        .arg(&format!("{}/extract.o", out_dir))
        .status()
        .expect("Failed to run nasm");

    println!("cargo:rustc-link-search=native={}", out_dir);
    println!("cargo:rustc-link-lib=static=amacal");
}
