# Tauri + Vue + TypeScript

This template should help get you started developing with Vue 3 and TypeScript in Vite. The template uses Vue 3 `<script setup>` SFCs, check out the [script setup docs](https://v3.vuejs.org/api/sfc-script-setup.html#sfc-script-setup) to learn more.

## Recommended IDE Setup

- [VS Code](https://code.visualstudio.com/) + [Vue - Official](https://marketplace.visualstudio.com/items?itemName=Vue.volar) + [Tauri](https://marketplace.visualstudio.com/items?itemName=tauri-apps.tauri-vscode) + [rust-analyzer](https://marketplace.visualstudio.com/items?itemName=rust-lang.rust-analyzer)

## setup
pnpm install
$env:RUST_BACKTRACE = "1"; $env:DEEPINDEX_LOG_LEVEL = "debug"; pnpm tauri dev

$env:RUST_BACKTRACE = "1"; $env:DEEPINDEX_LOG_LEVEL = "info"; cargo nextest run --manifest-path .\src-tauri\Cargo.toml


C:\Users\dongchao\AppData\Local\Temp
C:\Users\dongchao\AppData\Roaming\DeepIndex\data
