# Tauri + Vue + TypeScript

This template should help get you started developing with Vue 3 and TypeScript in Vite. The template uses Vue 3 `<script setup>` SFCs, check out the [script setup docs](https://v3.vuejs.org/api/sfc-script-setup.html#sfc-script-setup) to learn more.

## Recommended IDE Setup

- [VS Code](https://code.visualstudio.com/) + [Vue - Official](https://marketplace.visualstudio.com/items?itemName=Vue.volar) + [Tauri](https://marketplace.visualstudio.com/items?itemName=tauri-apps.tauri-vscode) + [rust-analyzer](https://marketplace.visualstudio.com/items?itemName=rust-lang.rust-analyzer)

## setup
pnpm install
$env:RUST_BACKTRACE = "1"; $env:DEEPINDEX_LOG_LEVEL = "info"; pnpm tauri dev

$env:RUST_BACKTRACE = "1"; $env:DEEPINDEX_LOG_LEVEL = "info"; cargo nextest run --manifest-path .\src-tauri\Cargo.toml


C:\Users\dongchao\AppData\Local\Temp
C:\Users\dongchao\AppData\Roaming\DeepIndex\data

git clone https://github.com/microsoft/vcpkg.git
./bootstrap-vcpkg.bat
./vcpkg integrate install

# 安装 tesseract OCR 库 (静态链接版本)
./vcpkg install tesseract:x64-windows-static

# 如果静态链接版本安装失败，可以尝试以下解决方案：
# 1. 更新 vcpkg 到最新版本
# git pull
# 
# 2. 清理构建缓存
# Remove-Item -Recurse -Force buildtrees\libarchive -ErrorAction SilentlyContinue
# Remove-Item -Recurse -Force buildtrees\tesseract -ErrorAction SilentlyContinue
#
# 3. 单独安装依赖项
# ./vcpkg install libarchive:x64-windows-static --debug
#
# 4. 如果仍然失败，可以使用动态链接版本 (但需要部署时包含 DLL)
# ./vcpkg install tesseract:x64-windows

C:\Program Files\Tesseract-OCR

  cargo:rerun-if-env-changed=LEPTONICA_INCLUDE_PATH
  cargo:rerun-if-env-changed=LEPTONICA_LINK_PATHS
  cargo:rerun-if-env-changed=LEPTONICA_LINK_LIBS

$env:LEPTONICA_INCLUDE_PATH = "C:\Program Files\Tesseract-OCR"; $env:LEPTONICA_LINK_PATHS = "C:\Program Files\Tesseract-OCR"; $env:LEPTONICA_LINK_LIBS = "C:\Program Files\Tesseract-OCR"; cargo test --package deepindex --lib -- ocr::tests::test_ocr --exact --show-output


SET VCPKG_DEFAULT_TRIPLET=x64-windows; .\vcpkg install leptonica