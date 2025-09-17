# DeepIndex

DeepIndex 是一个基于Vue+Rust编写的本地文件内容索引和搜索工具，它能够对本地文件进行深度索引，包括文件目录、文件名和文件内容，支持全文检索，让您快速找到所需信息。

下载： [DeepIndex](https://github.com/dongchao-1/DeepIndex/releases)

## 🚀 核心功能
### 🔍 精准搜索
- **全文检索**: 索引文件内容，支持关键词精确匹配
- **三列布局**: 文件名、文件路径、匹配内容，信息一目了然
- **分页加载**: 无限滚动加载搜索结果，优化大量数据展示

### ⚙️ 灵活配置
- **目录管理**: 自定义索引目录，精准控制索引范围
- **文件类型过滤**: 树形界面管理文件扩展名白名单，当前支持格式有：

| 文件类型 | 支持格式 | 提取方式 |
|---------|---------|---------|
| 文本文件 | `.txt`, `.md`, `.markdown` | 读取文本内容，按行拆分 |
| Office 文档 | `.docx`, `.pptx`, `.xlsx` | 解析文档结构，按段落拆分 |
| PDF 文档 | `.pdf` | PDF内容解析 |
| 图像文件 | `.png`, `.jpg`, `.jpeg`, `.gif`, `.bmp`, `.tiff`, `.webp` | OCR文字识别 |

### 🔧 高级特性
- **OCR 文字识别**: 基于 Tesseract 引擎，提取图像中的文字内容
- **增量索引**: 智能监控文件变化，仅索引修改内容
- **SQLite 存储**: 本地数据库存储，保证数据安全与查询性能

## 🏗️ 技术架构

### 前端技术栈
- **框架**: Vue 3 + TypeScript
- **UI 组件**: Element Plus
- **构建工具**: Vite

## 后端技术栈
- **框架**: Tauri 2.x
- **语言**: Rust
- **数据库**: SQLite
- **OCR 引擎**: Tesseract + Leptonica
- **文档解析**: quick-xml, lopdf

## 构建系统
- **任务管理**: cargo-make
- **依赖管理**: vcpkg (C++ 依赖)
- **测试框架**: cargo-nextest
- **CI/CD**: GitHub Actions

## �💻 开发
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

### 3. 编写代码
编写代码，添加测试用例

### 4. 运行测试
```powershell
cargo make test
$env:RUST_BACKTRACE="full"; $env:DEEPINDEX_LOG_LEVEL="debug"; cargo make test
```

### 5. 运行开发版本
```powershell
cargo make dev
$env:RUST_BACKTRACE="full"; $env:DEEPINDEX_LOG_LEVEL="debug"; cargo make dev
```

### 6. 检查test、clippy、fmt
```powershell
cargo make format
cargo make check
```

### 7. 构建生产版本
```powershell
cargo make release
```

### 重要路径
- dev应用数据: `%APPDATA%\DeepIndex`
- 测试临时目录: `%TEMP%`

---

**DeepIndex** - 让本地文件搜索变得简单高效 🔍✨
