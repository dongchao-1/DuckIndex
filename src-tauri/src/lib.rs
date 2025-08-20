// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
use tauri::Emitter;
use tauri::Manager;
use std::path::Path;
use std::sync::OnceLock;
use std::thread;
use std::time::Duration;
use once_cell::sync::OnceCell;
use std::sync::mpsc::{self, Sender, Receiver};

use crate::config::AppConfig;
use crate::indexer::Indexer;
use crate::sqlite::init_pool;
use crate::worker::Worker;

mod config;
mod reader;
mod sqlite;
mod indexer;
// mod indexer_tantivy;
mod worker;
mod test;

pub static CONFIG: OnceCell<AppConfig> = OnceCell::new();

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
    // std::thread::spawn(move || {
    //     Indexer::reset_indexer().unwrap();
    //     let reader = reader::CompositeReader::new();
    //     let indexer = Indexer::get_indexer().unwrap();
    //     let tx = get_tx();
    //     let data_paths = CONFIG.get().unwrap().data_path.clone();
    //     for data_path in &data_paths {
    //         let path = Path::new(data_path);
    //         if path.is_dir() {
    //             tx.send(format!("开始索引 {}", data_path)).unwrap();
    //             let cnt = count_files_recursive(path).unwrap();
    //             let mut dir_stat = DirStatistics {
    //                 dir_path: data_path.to_string(),
    //                 file_count: cnt,
    //                 index_count: 0,
    //             };
    //             index_dir(path, &reader, &indexer, tx, &mut dir_stat).unwrap();
    //             tx.send(format!("索引完成 {}", data_path)).unwrap();
    //         } else {
    //             eprintln!("Path does not exist: {}", data_path);
    //         }
    //     }
    // });
    let worker = Worker::new().unwrap();

    let data_paths = CONFIG.get().unwrap().data_path.clone();
    for data_path in data_paths {
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

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
                CONFIG.set(AppConfig::load(app.handle())?).unwrap();

                init_pool();

                Indexer::check_or_init().unwrap();
                Worker::check_or_init().unwrap();
                Worker::reset_running_tasks().unwrap();

                Worker::start_process().unwrap();

                let window = app.get_webview_window("main").unwrap();
                setup_index_task(window);

                Ok(())
            })
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![search, index_all_files])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

