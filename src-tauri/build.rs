use std::env;
use std::fs;

fn main() {
    // 静态链接
    // println!("cargo:rustc-flags=-C target-feature=+crt-static");

    // 获取当前工作目录，构建相对路径
    let current_dir = env::current_dir().expect("无法获取当前目录");
    let vcpkg_base = current_dir.parent().expect("无法获取父目录").join("vcpkg");
    if !vcpkg_base.is_dir() {
        panic!(
            "vcpkg_base is not a valid directory: {}",
            vcpkg_base.display()
        );
    }
    let vcpkg_include = vcpkg_base
        .join("installed")
        .join("x64-windows-static")
        .join("include");
    let vcpkg_lib = vcpkg_base
        .join("installed")
        .join("x64-windows-static")
        .join("lib");

    println!(
        "cargo:rustc-env=LEPTONICA_INCLUDE_PATH={}",
        vcpkg_include.display()
    );
    println!(
        "cargo:rustc-env=LEPTONICA_LINK_PATHS={}",
        vcpkg_lib.display()
    );
    println!("cargo:rustc-env=LEPTONICA_LINK_LIBS=leptonica-1.85.0");
    println!(
        "cargo:rustc-env=TESSERACT_INCLUDE_PATHS={}",
        vcpkg_include.display()
    );
    println!(
        "cargo:rustc-env=TESSERACT_LINK_PATHS={}",
        vcpkg_lib.display()
    );
    println!("cargo:rustc-env=TESSERACT_LINK_LIBS=tesseract55");

    // 读取目录下所有 .lib 文件
    if vcpkg_lib.is_dir() {
        println!("cargo:rustc-link-search=native={}", vcpkg_lib.display());
        match fs::read_dir(&vcpkg_lib) {
            Ok(entries) => {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if let Some(extension) = path.extension() {
                        if extension == "lib" {
                            if let Some(file_name) = path.file_name() {
                                if let Some(file_name_str) = file_name.to_str() {
                                    let lib_name = file_name_str.trim_end_matches(".lib");
                                    println!("cargo:rustc-link-lib=static={lib_name}");
                                }
                            }
                        }
                    }
                }
            }
            Err(e) => {
                panic!("Failed to read directory {}: {}", vcpkg_lib.display(), e);
            }
        }
    } else {
        panic!(
            "vcpkg_lib is not a valid directory: {}",
            vcpkg_lib.display()
        );
    }

    tauri_build::build()
}
