use std::env;
use std::fs;
use std::path::Path;

fn main() {
    // 获取当前工作目录，构建相对路径
    let current_dir = env::current_dir().expect("无法获取当前目录");
    let vcpkg_base = current_dir.parent().expect("无法获取父目录").join("vcpkg");
    if !vcpkg_base.is_dir() {
        panic!(
            "vcpkg_base is not a valid directory: {}",
            vcpkg_base.display()
        );
    }
    // let vcpkg_include = vcpkg_base
    //     .join("installed")
    //     .join("x64-windows")
    //     .join("include");
    // let vcpkg_lib = vcpkg_base
    //     .join("installed")
    //     .join("x64-windows")
    //     .join("lib");
    let vcpkg_bin = vcpkg_base
        .join("installed")
        .join("x64-windows")
        .join("bin");
    let out_dir = env::var("OUT_DIR").expect("OUT_DIR not set");
    let target_dir = Path::new(&out_dir).parent().unwrap().parent().unwrap().parent().unwrap();
    println!("cargo:info=Target directory: {}", target_dir.display());

    // println!(
    //     "cargo:rustc-env=LEPTONICA_INCLUDE_PATH={}",
    //     vcpkg_include.display()
    // );
    // println!(
    //     "cargo:rustc-env=LEPTONICA_LINK_PATHS={}",
    //     vcpkg_lib.display()
    // );
    // println!("cargo:rustc-env=LEPTONICA_LINK_LIBS=leptonica-1.85.0");
    // println!(
    //     "cargo:rustc-env=TESSERACT_INCLUDE_PATHS={}",
    //     vcpkg_include.display()
    // );
    // println!(
    //     "cargo:rustc-env=TESSERACT_LINK_PATHS={}",
    //     vcpkg_lib.display()
    // );
    // println!("cargo:rustc-env=TESSERACT_LINK_LIBS=tesseract55");

    // 读取目录下所有 .lib 文件
    if vcpkg_bin.is_dir() {
        match fs::read_dir(&vcpkg_bin) {
            Ok(entries) => {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if let Some(extension) = path.extension() {
                        if extension == "dll" {
                            if let Some(file_name) = path.file_name() {
                                let dest_path = target_dir.join(file_name);
                                fs::copy(&path, &dest_path).unwrap_or_else(|_| panic!("Failed to copy {} to {}", path.display(), dest_path.display()));
                            }
                        }
                    }
                }
            }
            Err(e) => {
                panic!("Failed to read directory {}: {}", vcpkg_bin.display(), e);
            }
        }
    } else {
        panic!(
            "vcpkg_bin is not a valid directory: {}",
            vcpkg_bin.display()
        );
    }

    tauri_build::build()
}
