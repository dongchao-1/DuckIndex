use ::log::info;
use serde::Serialize;
use tauri::RunEvent;
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
use crate::monitor::add_watched_path;
use crate::monitor::del_watched_path;
use crate::sqlite::close_pool;
use crate::sqlite::init_pool;
use crate::worker::TaskStatusStat;
use crate::worker::Worker;
use crate::monitor::get_monitor;

mod config;
mod dirs;
mod indexer;
mod log;
mod reader;
mod sqlite;
// mod indexer_tantivy;
mod test;
mod worker;
mod monitor;

#[derive(Debug, Clone, Serialize)]
struct TotalStatus {
    task_status_stat: TaskStatusStat,
    index_status_stat: IndexStatusStat,
}

fn setup_index_task(window: tauri::WebviewWindow) {
    thread::Builder::new()
        .name("status-updater".to_string())
        .spawn(move || {
            let worker = Worker::new().unwrap();
            let indexer = Indexer::new().unwrap();
            loop {
                let task_status_stat = worker.get_tasks_status().unwrap();
                let index_status_stat = indexer.get_index_status().unwrap();

                window.emit("status-update", TotalStatus {
                    task_status_stat,
                    index_status_stat,
                }).unwrap();
                thread::sleep(Duration::from_secs(1));
            }
        }).unwrap();
}

#[tauri::command]
fn add_index_path(path: &str) {
    let mut paths = Config::get_index_dir_paths().unwrap();
    paths.push(path.to_string());
    Config::set_index_dir_paths(paths).unwrap();

    let new_path = Path::new(path);
    let worker = Worker::new().unwrap();
    info!("开始索引目录: {}", new_path.display());
    worker.submit_index_all_files(&new_path).unwrap();

    add_watched_path(&new_path);
}

#[tauri::command]
fn del_index_path(path: &str) {
    let mut paths = Config::get_index_dir_paths().unwrap();
    paths.retain(|p| p != path);
    Config::set_index_dir_paths(paths).unwrap();

    let old_path = Path::new(path);
    let indexer = Indexer::new().unwrap();
    info!("开始删除目录: {}", old_path.display());
    indexer.delete_directory(&old_path).unwrap();

    del_watched_path(&old_path);
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
    info!("DeepIndex启动");
    setup_backend();

    info!("开始检查已有目录");
    thread::Builder::new()
        .name("initial-check-index-dir-paths".to_string())
        .spawn(|| {
            let worker = Worker::new().unwrap();
            Config::get_index_dir_paths().unwrap().iter().for_each(|path| {
                info!("开始检查目录: {}", path);
                worker.submit_index_all_files(Path::new(path)).unwrap();
            info!("目录检查完成: {}", path);
        });
    }).unwrap();

    info!("启动后台变更监听");
    get_monitor();

    info!("启动后台索引服务");
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
        .build(tauri::generate_context!())
        .expect("error while running tauri application")
        .run(|_app_handle, event| {
            if let RunEvent::Exit = event {
                close_pool();
            }
        });
}
