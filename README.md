# Tauri + Vue + TypeScript

This template should help get you started developing with Vue 3 and TypeScript in Vite. The template uses Vue 3 `<script setup>` SFCs, check out the [script setup docs](https://v3.vuejs.org/api/sfc-script-setup.html#sfc-script-setup) to learn more.



## 开发
### 推荐IDE和插件
- [VS Code](https://code.visualstudio.com/) + [Vue - Official](https://marketplace.visualstudio.com/items?itemName=Vue.volar) + [Tauri](https://marketplace.visualstudio.com/items?itemName=tauri-apps.tauri-vscode) + [rust-analyzer](https://marketplace.visualstudio.com/items?itemName=rust-lang.rust-analyzer)

### 1. 安装基础环境
* 安装rust: https://www.rust-lang.org/tools/install
* 安装node.js v22: https://nodejs.org/zh-cn/download
* 安装pnpm: https://pnpm.io/installation
* 安装llvm: https://releases.llvm.org/
* 安装cargo-make: `cargo install cargo-make`
* 安装cargo-nextest: `cargo install cargo-nextest`

### 2. 安装依赖
```powershell
cargo make install
```

### 3. 运行测试
```powershell
cargo make test
```

### 4. 运行开发版本
```powershell
$env:DEEPINDEX_LOG_LEVEL="debug"; cargo make dev
```

### 5. 构建生产版本
```powershell
cargo make release
```

### 重要路径
- dev应用数据: `%APPDATA%\DeepIndex`
- 测试临时目录: `%TEMP%`
