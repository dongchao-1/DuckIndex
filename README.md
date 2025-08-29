# Tauri + Vue + TypeScript

This template should help get you started developing with Vue 3 and TypeScript in Vite. The template uses Vue 3 `<script setup>` SFCs, check out the [script setup docs](https://v3.vuejs.org/api/sfc-script-setup.html#sfc-script-setup) to learn more.

## Recommended IDE Setup

- [VS Code](https://code.visualstudio.com/) + [Vue - Official](https://marketplace.visualstudio.com/items?itemName=Vue.volar) + [Tauri](https://marketplace.visualstudio.com/items?itemName=tauri-apps.tauri-vscode) + [rust-analyzer](https://marketplace.visualstudio.com/items?itemName=rust-lang.rust-analyzer)

## setup
pnpm install
$env:RUST_BACKTRACE = "1"; $env:DEEPINDEX_LOG_LEVEL = "info"; pnpm tauri dev
$env:RUST_BACKTRACE = "1"; $env:DEEPINDEX_LOG_LEVEL = "info"; cargo nextest run --manifest-path .\src-tauri\Cargo.toml


$env:LEPTONICA_INCLUDE_PATH = "C:\Users\dongchao\Code\vcpkg\installed\x64-windows\include\leptonica"; $env:LEPTONICA_LINK_PATHS = "C:\Users\dongchao\Code\vcpkg\installed\x64-windows\bin"; $env:LEPTONICA_LINK_LIBS = "C:\Users\dongchao\Code\vcpkg\installed\x64-windows\lib"; cargo nextest run --manifest-path .\src-tauri\Cargo.toml

# 构建生产版本
pnpm tauri build



C:\Users\dongchao\AppData\Local\Temp
C:\Users\dongchao\AppData\Roaming\DeepIndex\data

git clone https://github.com/microsoft/vcpkg.git
./bootstrap-vcpkg.bat
./vcpkg integrate install

# 安装 tesseract OCR 库 (静态链接版本)
./vcpkg install tesseract:x64-windows
./vcpkg install tesseract:x64-windows


SET RUSTFLAGS=-Ctarget-feature=+crt-static; cargo test --package deepindex --lib -- ocr::tests::test_ocr --exact --show-output
