use std::env;
use std::fs;
use std::path::Path;

fn main() {
    // 告诉 Cargo，当这个脚本改变时重新运行
    // println!("cargo:rerun-if-changed=build.rs");
    let tesseract_bin_path = env::var("TESSERACT_BIN_PATH").expect("TESSERACT_BIN_PATH not set");
    let tesseract_bin_path = Path::new(&tesseract_bin_path);

    // 获取 Cargo 的目标构建目录
    let out_dir = env::var("OUT_DIR").expect("OUT_DIR not set");
    println!("cargo:info=OUT_DIR: {out_dir}");

    // OUT_DIR 通常是: target/debug/build/your-crate-xxx/out
    // 我们需要回到 target/debug 目录
    let target_dir = Path::new(&out_dir).join("../../..");

    println!("cargo:info=Target directory: {}", target_dir.display());

    // 要复制的 DLL 文件列表
    let dlls_to_copy = [
        "archive.dll",
        "zstd.dll",
        "zlib1.dll",
        "turbojpeg.dll",
        "tiff.dll",
        "tesseract55.dll",
        "openjp2.dll",
        "lz4.dll",
        "libwebpmux.dll",
        "libwebpdemux.dll",
        "libwebpdecoder.dll",
        "libwebp.dll",
        "libssl-3-x64.dll",
        "libsharpyuv.dll",
        "libpng16.dll",
        "liblzma.dll",
        "libcurl.dll",
        "libcrypto-3-x64.dll",
        "leptonica-1.85.0.dll",
        "legacy.dll",
        "jpeg62.dll",
        "gif.dll",
        "bz2.dll",
    ];

    for dll_name in &dlls_to_copy {
        let src_path = tesseract_bin_path.join(dll_name);
        // 直接复制到 target/debug 或 target/release 目录
        let dest_path = target_dir.join(dll_name);
        fs::create_dir_all(&target_dir).unwrap_or_else(|_| panic!("Failed to create directory {}", target_dir.display()));
        fs::copy(&src_path, &dest_path).unwrap_or_else(|_| panic!("Failed to copy {} to {}", src_path.display(), dest_path.display()));
    }

    tauri_build::build()
}
