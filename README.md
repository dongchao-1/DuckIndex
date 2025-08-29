# Tauri + Vue + TypeScript

This template should help get you started developing with Vue 3 and TypeScript in Vite. The template uses Vue 3 `<script setup>` SFCs, check out the [script setup docs](https://v3.vuejs.org/api/sfc-script-setup.html#sfc-script-setup) to learn more.

## Recommended IDE Setup

- [VS Code](https://code.visualstudio.com/) + [Vue - Official](https://marketplace.visualstudio.com/items?itemName=Vue.volar) + [Tauri](https://marketplace.visualstudio.com/items?itemName=tauri-apps.tauri-vscode) + [rust-analyzer](https://marketplace.visualstudio.com/items?itemName=rust-lang.rust-analyzer)

## Setup

### 1. 安装前端依赖
```powershell
pnpm install
```

### 2. 设置vcpkg和tesseract
```powershell
# 克隆vcpkg
git clone https://github.com/microsoft/vcpkg.git
cd vcpkg
./bootstrap-vcpkg.bat
./vcpkg integrate install

# 安装tesseract OCR库
./vcpkg install tesseract:x64-windows
```

### 3. 运行开发版本
```powershell
$env:RUST_BACKTRACE = "1"; $env:DEEPINDEX_LOG_LEVEL = "info"; pnpm tauri dev
```

### 4. 运行测试（包含OCR测试）
```powershell
# 设置环境变量和PATH
# $env:PATH = "C:\Users\dongchao\Code\vcpkg\installed\x64-windows\bin;$env:PATH"

$env:RUSTFLAGS = "-Ctarget-feature=+crt-static"
$env:TESSERACT_BIN_PATH = "C:\Users\dongchao\Code\vcpkg\installed\x64-windows\bin"
$env:LEPTONICA_INCLUDE_PATH = "C:\Users\dongchao\Code\vcpkg\installed\x64-windows\include"
$env:LEPTONICA_LINK_PATHS = "C:\Users\dongchao\Code\vcpkg\installed\x64-windows\lib"
$env:LEPTONICA_LINK_LIBS = "leptonica-1.85.0"
$env:TESSERACT_INCLUDE_PATHS = "C:\Users\dongchao\Code\vcpkg\installed\x64-windows\include"
$env:TESSERACT_LINK_PATHS = "C:\Users\dongchao\Code\vcpkg\installed\x64-windows\lib"
$env:TESSERACT_LINK_LIBS = "tesseract55"

# 运行所有测试
cargo nextest run --manifest-path .\src-tauri\Cargo.toml

# 如果使用标准cargo test命令（也显示输出）
cargo test --manifest-path .\src-tauri\Cargo.toml --package deepindex --lib -- ocr::test::main --exact --show-output
```

### 5. 构建生产版本
```powershell
pnpm tauri build
```

## 重要路径

- 临时目录: `C:\Users\dongchao\AppData\Local\Temp`
- 应用数据: `C:\Users\dongchao\AppData\Roaming\DeepIndex\data`
