use ::log::info;
use serde::Serialize;
use std::path::Path;
use std::thread;
use std::time::Duration;
use tauri::Emitter;
use tauri::Manager;

use crate::config::Config;
use crate::indexer::IndexStatusStat;
use crate::indexer::Indexer;
use crate::indexer::SearchResultDirectory;
use crate::indexer::SearchResultFile;
use crate::indexer::SearchResultItem;
use crate::log::init_logger;
use crate::sqlite::init_pool;
use crate::worker::TaskStatusStat;
use crate::worker::Worker;

mod config;
mod dirs;
mod indexer;
mod log;
mod reader;
mod sqlite;
// mod indexer_tantivy;
mod test;
mod worker;

#[derive(Debug, Clone, Serialize)]
struct TotalStatus {
    task_status_stat: TaskStatusStat,
    index_status_stat: IndexStatusStat,
}

fn setup_index_task(window: tauri::WebviewWindow) {
    std::thread::spawn(move || loop {
        let worker = Worker::new().unwrap();
        let indexer = Indexer::new().unwrap();
        let task_status_stat = worker.get_tasks_status().unwrap();
        let index_status_stat = indexer.get_index_status().unwrap();

        window.emit("index-task-update", TotalStatus {
            task_status_stat,
            index_status_stat,
        }).unwrap();
        thread::sleep(Duration::from_secs(1));
    });
}

#[tauri::command]
fn add_index_path(path: &str) {
    let mut paths = Config::get_index_dir_paths().unwrap();
    paths.push(path.to_string());
    Config::set_index_dir_paths(paths).unwrap();

    let worker = Worker::new().unwrap();
    info!("开始索引目录: {}", path);
    worker.submit_index_all_files(Path::new(&path)).unwrap();
}

#[tauri::command]
fn del_index_path(path: &str) {
    let mut paths = Config::get_index_dir_paths().unwrap();
    paths.retain(|p| p != path);
    Config::set_index_dir_paths(paths).unwrap();

    let indexer = Indexer::new().unwrap();
    info!("开始删除目录: {}", path);
    indexer.delete_directory(Path::new(&path)).unwrap();
}

#[tauri::command]
fn search_directory(query: &str, offset: usize, limit: usize) -> Vec<SearchResultDirectory> {
    let indexer = Indexer::new().unwrap();
    indexer.search_directory(query, offset, limit).unwrap()
}

#[tauri::command]
fn search_file(query: &str, offset: usize, limit: usize) -> Vec<SearchResultFile> {
    let indexer = Indexer::new().unwrap();
    indexer.search_file(query, offset, limit).unwrap()
}

#[tauri::command]
fn search_item(query: &str, offset: usize, limit: usize) -> Vec<SearchResultItem> {
    let indexer = Indexer::new().unwrap();
    indexer.search_item(query, offset, limit).unwrap()
}

#[tauri::command]
fn get_index_dir_paths() -> Vec<String> {
    Config::get_index_dir_paths().unwrap_or_else(|_| vec![])
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
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let window = app.get_webview_window("main").unwrap();
            setup_index_task(window);
            Ok(())
        })
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            search_directory,
            search_file,
            search_item,
            add_index_path,
            del_index_path,
            get_index_dir_paths
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
