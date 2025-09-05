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

# 设置环境变量和PATH
设置 .vscode\settings.json 中vcpkg目录
```

### 3. 手动运行测试（命令行）
```powershell

# 安装nextest
cargo install nextest

# 运行所有测试
cargo nextest run --manifest-path .\src-tauri\Cargo.toml
```

### 4. 运行开发版本
```powershell
$env:DEEPINDEX_LOG_LEVEL="debug"; pnpm tauri dev
```

### 5. 构建生产版本
```powershell
pnpm tauri build
```

## 重要路径

- 临时目录: `C:\Users\dongchao\AppData\Local\Temp`
- 应用数据: `C:\Users\dongchao\AppData\Roaming\DeepIndex\data`
