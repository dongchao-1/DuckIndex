# CARGO_MAKE_WORKING_DIRECTORY 在 justfile 中通常直接用 '.' 代表当前目录
# 但为了保持与 VCPKG 路径的兼容性，我们显式定义它
set shell := ["powershell.exe", "-NoProfile", "-Command"]
VCPKG := "C:\\Program Files\\Microsoft Visual Studio\\2022\\Enterprise\\VC\\vcpkg\\vcpkg.exe"

export LIBCLANG_PATH := "C:\\Program Files\\Microsoft Visual Studio\\2022\\Enterprise\\VC\\Tools\\Llvm\\x64\\bin"
export LEPTONICA_INCLUDE_PATH := `$PWD.Path`+"\\vcpkg_installed\\x64-windows\\include"
export LEPTONICA_LINK_PATHS := `$PWD.Path`+"\\vcpkg_installed\\x64-windows\\lib"
export LEPTONICA_LINK_LIBS := "leptonica-1.85.0"
export TESSERACT_INCLUDE_PATHS := `$PWD.Path`+"\\vcpkg_installed\\x64-windows\\include"
export TESSERACT_LINK_PATHS := `$PWD.Path`+"\\vcpkg_installed\\x64-windows\\lib"
export TESSERACT_LINK_LIBS := "tesseract55"
export DUCKDB_INCLUDE_DIR := `$PWD.Path`+"\\vcpkg_installed\\x64-windows\\include"
export DUCKDB_LIB_DIR := `$PWD.Path`+"\\vcpkg_installed\\x64-windows\\lib"

echo-env:
    @echo "LEPTONICA_INCLUDE_PATH={{LEPTONICA_INCLUDE_PATH}}"
    @echo "LEPTONICA_LINK_PATHS={{LEPTONICA_LINK_PATHS}}"
    @echo "LEPTONICA_LINK_LIBS={{LEPTONICA_LINK_LIBS}}"
    @echo "TESSERACT_INCLUDE_PATHS={{TESSERACT_INCLUDE_PATHS}}"
    @echo "TESSERACT_LINK_PATHS={{TESSERACT_LINK_PATHS}}"
    @echo "TESSERACT_LINK_LIBS={{TESSERACT_LINK_LIBS}}"
    @echo "LIBCLANG_PATH={{LIBCLANG_PATH}}"
    @echo "DUCKDB_INCLUDE_DIR={{DUCKDB_INCLUDE_DIR}}"
    @echo "DUCKDB_LIB_DIR={{DUCKDB_LIB_DIR}}"

# 安装包
# ==================================

install-vcpkg-pkgs:
    & "{{VCPKG}}" install

install-cargo-pkgs:
    cargo install cargo-nextest

install-npm-pkgs:
    npm install


# 构建与检查任务
# ==================================

build:
    cargo build --manifest-path ./src-tauri/Cargo.toml

test:
    cargo nextest run --manifest-path ./src-tauri/Cargo.toml

test-debug:
    $env:RUST_BACKTRACE="full"; $env:DUCKINDEX_LOG_LEVEL="debug"; cargo nextest run --manifest-path ./src-tauri/Cargo.toml

clippy:
    cargo clippy --manifest-path ./src-tauri/Cargo.toml --all-targets --all-features -- -D warnings

format:
    cargo fmt --manifest-path ./src-tauri/Cargo.toml

format-check:
    cargo fmt --manifest-path ./src-tauri/Cargo.toml --check


# 运行与发布任务
# ==================================

dev:
    npm run tauri dev

dev-debug:
    $env:RUST_BACKTRACE="full"; $env:DUCKINDEX_LOG_LEVEL="debug"; npm run tauri dev

release:
    npm run tauri build


# 清理任务
# ==================================

clean-cargo-pkgs:
    cargo clean --manifest-path ./src-tauri/Cargo.toml

clean-vcpkg-pkgs:
    try { Remove-Item -Recurse -Force -ErrorAction SilentlyContinue vcpkg_installed } finally { exit 0 }

clean-npm-pkgs:
    try { Remove-Item -Recurse -Force -ErrorAction SilentlyContinue node_modules, dist } finally { exit 0 }


# 任务组 (Dependencies)
# ==================================

# 任务组：安装所有依赖
install: install-vcpkg-pkgs install-cargo-pkgs install-npm-pkgs

# 任务组：执行检查
check: test clippy format-check

# 任务组：清理所有
clean: clean-vcpkg-pkgs clean-cargo-pkgs clean-npm-pkgs
