use std::process::Command;
use std::env;

fn main() {
    let project_dir = env::current_dir().unwrap();
    let sa_dir = project_dir.join("sa");

    // Rebuild the C files if they change
    println!("cargo:rerun-if-changed=sa/payload.c");
    println!("cargo:rerun-if-changed=sa/mosaic.defs");
    println!("cargo:rerun-if-changed=sa/Makefile");

    // Run Make in the sa directory
    let status = Command::new("make")
        .current_dir(&sa_dir)
        .status()
        .expect("Failed to run make for scripting addition");
    
    if !status.success() {
        panic!("Make failed to build the scripting addition");
    }

    // Link the generated static library
    println!("cargo:rustc-link-search=native={}", sa_dir.display());
    println!("cargo:rustc-link-lib=static=mosaicUser");
}
