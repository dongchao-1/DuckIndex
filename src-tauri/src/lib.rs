// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
use tauri::Emitter;
use tauri::Manager;
use std::fs;
use std::path::Path;
use once_cell::sync::OnceCell;

use crate::config::AppConfig;
use crate::indexer::Indexer;

mod config;
mod reader;
mod indexer;
mod test;

#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

pub static CONFIG: OnceCell<AppConfig> = OnceCell::new();

fn setup_index_task(window: tauri::WebviewWindow) {
    std::thread::spawn(move || {
        loop {
            std::thread::sleep(std::time::Duration::from_secs(1));
            // 向前端发送事件
            // println!("Sending index task update to frontend");
            window.emit("index-task-update", "任务更新").unwrap();
        }
    });
}

#[tauri::command]
fn index_all_files() {
    Indexer::reset_indexer().unwrap();
    let data_paths = CONFIG.get().unwrap().data_path.clone();
    for data_path in &data_paths {
        let path = Path::new(data_path);
        if path.exists() {
            index_dir(path).unwrap();
        } else {
            eprintln!("Path does not exist: {}", data_path);
        }
    }
}

fn index_dir(path:&Path) -> Result<(), Box<dyn std::error::Error>> {
    let reader = reader::CompositeReader::new();
    let indexer = Indexer::get_indexer()?;

    let entries = fs::read_dir(path)?;
    for entry in entries {
        let entry = entry?;
        let file_type = entry.file_type().unwrap();
        if file_type.is_file() {
            // 处理文件
            let file = entry.path();
            let items = reader.read(&file)?;
            indexer.write_items(&file, items);
        } else if file_type.is_dir() {
            // 递归处理子目录
            index_dir(&entry.path())?;
        }
    }
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
                CONFIG.set(AppConfig::load(app.handle())?).unwrap();

                let window = app.get_webview_window("main").unwrap();
                setup_index_task(window);
                Ok(())
            })
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![greet, index_all_files])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
