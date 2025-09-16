use ::log::info;
use anyhow::Result;
use serde::Serialize;
use std::future::Future;
use std::path::Path;
use std::thread;
use tauri::{async_runtime, RunEvent};
use thiserror::Error;

use crate::config::Config;
use crate::indexer::IndexStatusStat;
use crate::indexer::Indexer;
use crate::indexer::SearchResultDirectory;
use crate::indexer::SearchResultFile;
use crate::indexer::SearchResultItem;
use crate::log::init_logger;
use crate::monitor::add_watched_path;
use crate::monitor::del_watched_path;
use crate::monitor::get_monitor;
use crate::sqlite::{check_or_init_db, close_pool, init_pool};
use crate::worker::{TaskStatusStat, Worker};

mod config;
mod dirs;
mod indexer;
mod log;
mod reader;
mod sqlite;
// mod indexer_tantivy;
mod monitor;
mod test;
mod utils;
mod worker;

#[derive(Debug, Error)]
pub enum TauriError {
    #[error(transparent)]
    Anyhow(#[from] anyhow::Error),
}

impl serde::Serialize for TauriError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::ser::Serializer,
    {
        let TauriError::Anyhow(ref err) = self;
        serializer.serialize_str(&format!("{}\nBacktrace:\n{}", self, err.backtrace()))
    }
}

type TauriResult<T> = std::result::Result<T, TauriError>;

async fn tauri_spawn<T, Fut>(fut: Fut) -> TauriResult<T>
where
    Fut: Future<Output = Result<T, anyhow::Error>> + Send + 'static,
    T: Send + 'static,
{
    match async_runtime::spawn(fut).await {
        Ok(inner) => inner.map_err(TauriError::Anyhow),
        Err(join_err) => Err(TauriError::Anyhow(join_err.into())),
    }
}

#[tauri::command]
async fn add_index_path(path: String) -> TauriResult<()> {
    tauri_spawn(async move {
        // TODO 检查是否重复、覆盖
        let new_path = Path::new(&path);
        add_watched_path(new_path)?;

        let worker = Worker::new()?;
        info!("开始索引目录: {}", new_path.display());
        worker.submit_index_all_files(new_path)?;

        let mut paths = Config::get_index_dir_paths()?;
        paths.push(path.clone());
        Config::set_index_dir_paths(paths)?;

        Ok(())
    })
    .await
}

#[tauri::command]
async fn del_index_path(path: String) -> TauriResult<()> {
    tauri_spawn(async move {
        let old_path = Path::new(&path);
        del_watched_path(old_path)?;

        let worker = Worker::new()?;
        info!("开始删除目录: {}", old_path.display());
        worker.submit_delete_all_files(old_path)?;

        let mut paths = Config::get_index_dir_paths()?;
        paths.retain(|p| p != &path);
        Config::set_index_dir_paths(paths)?;

        Ok(())
    })
    .await
}

#[tauri::command]
async fn search_directory(
    query: String,
    offset: usize,
    limit: usize,
) -> TauriResult<Vec<SearchResultDirectory>> {
    tauri_spawn(async move {
        let indexer = Indexer::new()?;
        indexer.search_directory(&query, offset, limit)
    })
    .await
}

#[tauri::command]
async fn search_file(
    query: String,
    offset: usize,
    limit: usize,
) -> TauriResult<Vec<SearchResultFile>> {
    tauri_spawn(async move {
        let indexer = Indexer::new()?;
        indexer.search_file(&query, offset, limit)
    })
    .await
}

#[tauri::command]
async fn search_item(
    query: String,
    offset: usize,
    limit: usize,
) -> TauriResult<Vec<SearchResultItem>> {
    tauri_spawn(async move {
        let indexer = Indexer::new()?;
        indexer.search_item(&query, offset, limit)
    })
    .await
}

#[tauri::command]
async fn get_index_dir_paths() -> TauriResult<Vec<String>> {
    tauri_spawn(async move { Config::get_index_dir_paths() }).await
}

#[derive(Debug, Clone, Serialize)]
struct TotalStatus {
    task_status_stat: TaskStatusStat,
    index_status_stat: IndexStatusStat,
}

#[tauri::command]
async fn get_status() -> TauriResult<TotalStatus> {
    tauri_spawn(async move {
        let worker = Worker::new()?;
        let indexer = Indexer::new()?;
        let task_status_stat = worker.get_tasks_status()?;
        let index_status_stat = indexer.get_index_status()?;

        Ok(TotalStatus {
            task_status_stat,
            index_status_stat,
        })
    })
    .await
}

pub fn setup_backend() {
    init_logger();
    init_pool();

    check_or_init_db().unwrap();
    Worker::reset_running_tasks().unwrap();
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    setup_backend();

    info!("开始检查已有目录");
    thread::Builder::new()
        .name("initial-check-index-dir-paths".to_string())
        .spawn(|| {
            let worker = Worker::new().unwrap();
            Config::get_index_dir_paths()
                .unwrap()
                .iter()
                .for_each(|path| {
                    info!("开始检查目录: {path}");
                    worker.submit_index_all_files(Path::new(path)).unwrap();
                    info!("目录检查完成: {path}");
                });
        })
        .unwrap();

    info!("启动后台变更监听");
    get_monitor();

    info!("启动后台索引服务");
    Worker::start_process().unwrap();

    info!("启动tauri前端服务");
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            search_directory,
            search_file,
            search_item,
            add_index_path,
            del_index_path,
            get_index_dir_paths,
            get_status,
        ])
        .build(tauri::generate_context!())
        .expect("error while running tauri application")
        .run(|_app_handle, event| {
            if let RunEvent::Exit = event {
                close_pool();
            }
        });
}
