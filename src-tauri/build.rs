use find_msvc_tools::find;
use std::env;
use std::fs;
use std::path::Path;

fn find_dll(dll: &str) -> String {
    find("x64", dll)
        .expect("Failed to find msvcp140.dll")
        .get_program()
        .to_str()
        .expect("Failed to convert path to string")
        .to_string()
}

fn main() {
    // 获取当前工作目录，构建相对路径
    let current_dir = env::current_dir().expect("无法获取当前目录");

    let out_dir = env::var("OUT_DIR").expect("OUT_DIR not set");
    let target_dir = Path::new(&out_dir)
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .parent()
        .unwrap();
    println!("cargo:info=Target directory: {}", target_dir.display());

    // CRT依赖
    let crt_dlls = ["msvcp140.dll", "vcruntime140.dll", "vcruntime140_1.dll"];
    for dll in crt_dlls {
        let dll_path = find_dll(dll);
        println!("cargo:info=Found {dll}, {dll_path}");
        let dest_path = target_dir.join(dll);
        fs::copy(&dll_path, &dest_path)
            .unwrap_or_else(|_| panic!("Failed to copy {} to {}", dll_path, dest_path.display()));
    }

    // vcpkg依赖
    let vcpkg_base = current_dir
        .parent()
        .expect("无法获取父目录")
        .join("vcpkg_installed");
    if !vcpkg_base.is_dir() {
        panic!(
            "vcpkg_base is not a valid directory: {}",
            vcpkg_base.display()
        );
    }
    let vcpkg_bin = vcpkg_base.join("x64-windows").join("bin");
    if vcpkg_bin.is_dir() {
        match fs::read_dir(&vcpkg_bin) {
            Ok(entries) => {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if let Some(extension) = path.extension() {
                        if extension == "dll" {
                            if let Some(file_name) = path.file_name() {
                                let dest_path = target_dir.join(file_name);
                                fs::copy(&path, &dest_path).unwrap_or_else(|_| {
                                    panic!(
                                        "Failed to copy {} to {}",
                                        path.display(),
                                        dest_path.display()
                                    )
                                });
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
