use ::log::info;
// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
use tauri::Emitter;
use tauri::Manager;
use std::path::Path;
use std::thread;
use std::time::Duration;

use crate::config::Config;
use crate::log::init_logger;
use crate::indexer::Indexer;
use crate::sqlite::init_pool;
use crate::worker::Worker;

mod dirs;
mod config;
mod log;
mod reader;
mod sqlite;
mod indexer;
// mod indexer_tantivy;
mod worker;
mod test;

fn setup_index_task(window: tauri::WebviewWindow) {
    std::thread::spawn(move || {
        loop {
            // let msg = rx.recv().unwrap();
            let worker = Worker::new().unwrap();
            let msg = worker.get_tasks_status().unwrap();
            // println!("Sending index task update to frontend");
            window.emit("index-task-update", format!("{}", msg)).unwrap();
            thread::sleep(Duration::from_secs(1));
        }
    });
}


#[tauri::command]
fn index_all_files() -> String {
    let worker = Worker::new().unwrap();

    let data_paths = Config::get_index_dir_paths().unwrap();
    for data_path in data_paths {
        info!("开始索引目录: {}", data_path);
        worker.submit_index_all_files(Path::new(&data_path)).unwrap();
    }

    "开始重建索引".to_string()
}


#[tauri::command]
fn search(query: &str) -> String {
    let indexer = Indexer::new().unwrap();
    let results = indexer.search(query, 10).unwrap();
    format!("Found {} results: {:?}", results.len(), results)
}

pub fn setup_backend() {
    init_logger();
    init_pool();

    Config::check_or_init().unwrap();
    Indexer::check_or_init().unwrap();
    Worker::check_or_init().unwrap();
    Worker::reset_running_tasks().unwrap();
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    setup_backend();

    info!("启动后台服务");
    Worker::start_process().unwrap();

    info!("启动tauri前端服务");
    tauri::Builder::default()
        .setup(|app| {
                let window = app.get_webview_window("main").unwrap();
                setup_index_task(window);
                Ok(())
            })
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![search, index_all_files])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

