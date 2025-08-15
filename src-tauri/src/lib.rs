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
mod indexer_tantivy;
mod test;

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
fn index_all_files() -> String {
    Indexer::reset_indexer().unwrap();
    let reader = reader::CompositeReader::new();
    let indexer = Indexer::get_indexer().unwrap();

    let data_paths = CONFIG.get().unwrap().data_path.clone();
    for data_path in &data_paths {
        let path = Path::new(data_path);
        if path.exists() {
            index_dir(path, &reader, &indexer).unwrap();
        } else {
            eprintln!("Path does not exist: {}", data_path);
        }
    }
    "索引已重建".to_string()
}


#[tauri::command]
fn search(query: &str) -> String {
    let indexer = Indexer::get_indexer().unwrap();
    let results = indexer.search(query, 10).unwrap();
    format!("Found {} results: {:?}", results.len(), results)
}

fn index_dir(path:&Path, reader: &reader::CompositeReader, indexer: &Indexer) -> Result<(), Box<dyn std::error::Error>> {
    println!("Indexing directory: {}", path.display());
    indexer.write_directory(path).unwrap();
    let entries = fs::read_dir(path)?;
    for entry in entries {
        let entry = entry?;
        let file_type = entry.file_type().unwrap();
        if file_type.is_file() {
            // 处理文件
            println!("Indexing file: {}", entry.path().display());
            let file = entry.path();
            let items = reader.read(&file)?;
            indexer.write_file_items(&file, items).unwrap();
        } else if file_type.is_dir() {
            // 递归处理子目录
            index_dir(&entry.path(), reader, indexer).unwrap();
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
        .invoke_handler(tauri::generate_handler![search, index_all_files])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}


// #[cfg(test)]
// mod tests {
//     use super::*;
//     use crate::test::test::TestEnv;

//     #[test]
//     fn test_index_all_files() {
//         let _env = TestEnv::new();
//         index_all_files();
//         let indexer = Indexer::get_indexer().unwrap();
//         let result = indexer.search("is", 10).unwrap();
//         assert_eq!(result.len(), 1);
//     }

// }
