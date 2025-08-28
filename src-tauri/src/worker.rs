use anyhow::{anyhow, Result};
use chrono::Local;
use log::debug;
use log::error;
use log::info;
use rusqlite::params;
use serde::Serialize;
use std::collections::HashSet;
use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::str::FromStr;
use std::thread;
use std::time::Duration;
use strum::Display;
use strum::EnumString;

use crate::indexer::Indexer;
use crate::reader::CompositeReader;
use crate::sqlite::get_conn;

pub struct Worker {
    indexer: Indexer,
    reader: CompositeReader,
}

#[derive(Debug, PartialEq, EnumString, Display)]
enum TaskType {
    #[strum(to_string = "DIRECTORY")]
    DIRECTORY,
    #[strum(to_string = "FILE")]
    FILE,
}

#[derive(Debug, PartialEq, EnumString, Display)]
enum TaskStatus {
    #[strum(to_string = "PENDING")]
    PENDING,
    #[strum(to_string = "RUNNING")]
    RUNNING,
    #[strum(to_string = "FAILED")]
    FAILED,
}

#[derive(Debug, Clone, Serialize)]
pub struct TaskStatusStat {
    pub pending: usize,
    pub running: usize,
    pub failed: usize,
    pub running_tasks: Vec<String>,
    pub failed_tasks: Vec<String>,
}

impl Worker {
    pub fn check_or_init() -> Result<()> {
        if let Err(_) = Self::check_worker_init() {
            Self::reset_worker()?;
        }
        Ok(())
    }

    fn check_worker_init() -> Result<()> {
        let conn = get_conn()?;
        let row = conn
            .query_one("select version from worker_version", [], |row| {
                row.get::<_, String>(0)
            })
            .map_err(|e| anyhow!("Worker not initialized: {}", e))?;

        if row != "0.1" {
            return Err(anyhow!(
                "Worker version mismatch: expected 0.1, found {}",
                row
            ));
        }
        Ok(())
    }

    fn reset_worker() -> Result<()> {
        let conn = get_conn()?;
        conn.execute_batch(
            r"
            DROP TABLE IF EXISTS tasks;
            CREATE TABLE tasks (
                id INTEGER PRIMARY KEY,
                type TEXT NOT NULL,
                path TEXT NOT NULL,
                status TEXT NOT NULL,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                UNIQUE (type, path)
            );
            DROP TABLE IF EXISTS worker_version;
            CREATE TABLE worker_version (
                version TEXT
            );
            INSERT INTO worker_version (version) VALUES ('0.1');
        ",
        )?;
        Ok(())
    }

    pub fn reset_running_tasks() -> Result<()> {
        let conn = get_conn()?;
        conn.execute(
            "UPDATE tasks SET status = ?1, updated_at = ?2 WHERE status = ?3",
            params![
                TaskStatus::PENDING.to_string(),
                Local::now().to_rfc3339(),
                TaskStatus::RUNNING.to_string()
            ],
        )?;
        Ok(())
    }

    pub fn new() -> Result<Worker> {
        let indexer = Indexer::new()?;
        let reader = CompositeReader::new()?;
        Ok(Worker { indexer, reader })
    }

    fn add_task(&self, task_type: &TaskType, path: &Path) -> Result<i64> {
        let conn = get_conn()?;

        let path = path.to_str().unwrap().to_string();
        let now = Local::now().to_rfc3339();
        let id = conn.query_one(
            r"INSERT INTO tasks (type, path, status, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5) ON CONFLICT(type, path) 
                DO UPDATE SET status = ?3, updated_at = ?5 RETURNING id",
            params![
                task_type.to_string(),
                path,
                TaskStatus::PENDING.to_string(),
                now,
                now
            ],
            |row| {
                let id = row.get::<_, i64>(0)?;
                Ok(id)
            }
        )?;
        Ok(id)
    }

    fn split_dir_contents(&self, path: &Path) -> Result<(HashSet<PathBuf>, HashSet<PathBuf>)> {
        let mut dirs: HashSet<PathBuf> = HashSet::new();
        let mut files: HashSet<PathBuf> = HashSet::new();

        for entry in fs::read_dir(path)? {
            let entry = entry?;
            let path = entry.path();

            if path.is_dir() {
                dirs.insert(path);
            } else if path.is_file() {
                files.insert(path);
            }
        }

        Ok((dirs, files))
    }

    pub fn submit_index_all_files(&self, path: &Path) -> Result<()> {
        if path.exists() {
            if path.is_dir() {
                if let Ok(index_dir) = self.indexer.get_directory(path) {
                    // 数据库已经有这个目录了
                    let modified_time = self.indexer.get_modified_time(path)?;
                    if index_dir.modified_time != modified_time {
                        info!(
                            "目录索引过，但目录时间发生变更。目录: {} 原时间: {} 现时间:{}",
                            path.display(),
                            index_dir.modified_time,
                            modified_time
                        );
                        self.add_task(&TaskType::DIRECTORY, path)?;
                        info!("目录时间已更新。目录: {}", path.display());
                        // 目录修改了
                        let (index_sub_dirs, index_sub_files) =
                            self.indexer.get_sub_directories_and_files(path)?;
                        let (current_sub_dirs, current_sub_files) = self.split_dir_contents(path)?;

                        let index_sub_dirs = HashSet::from_iter(
                            index_sub_dirs
                                .iter()
                                .map(|p| Path::new(&p.path).to_path_buf()),
                        );
                        let index_sub_files = HashSet::from_iter(
                            index_sub_files
                                .iter()
                                .map(|p| Path::new(&p.path).join(&p.name).to_path_buf()),
                        );

                        for dir in index_sub_dirs.difference(&current_sub_dirs) {
                            // 删除的目录
                            info!("删除目录索引: {}", dir.display());
                            debug!("index_sub_dirs: {:?}", index_sub_dirs);
                            debug!("current_sub_dirs: {:?}", current_sub_dirs);
                            self.indexer.delete_directory(dir)?;
                        }
                        for file in index_sub_files.difference(&current_sub_files) {
                            // 删除的文件
                            info!("删除文件索引: {}", file.display());
                            debug!("index_sub_files: {:?}", index_sub_files);
                            debug!("current_sub_files: {:?}", current_sub_files);
                            self.indexer.delete_file(file)?;
                        }
                    }
                } else {
                    // 数据库中没有这个目录
                    info!("目录未索引，添加任务。目录: {}", path.display());
                    self.add_task(&TaskType::DIRECTORY, path)?;
                }

                for entry in fs::read_dir(path)? {
                    let entry = entry?;
                    let path = entry.path();

                    if path.is_file() {
                        if let Ok(index_file) = self.indexer.get_file(&path) {
                            let modified_time = self.indexer.get_modified_time(&path)?;
                            if index_file.modified_time == modified_time {
                                continue;
                            } else {
                                info!(
                                    "文件索引过，但文件时间发生变更。文件: {} 原时间: {} 现时间:{}",
                                    path.display(),
                                    index_file.modified_time,
                                    modified_time
                                );
                                self.indexer.delete_file(&path)?;
                                if self.reader.supports(&path)? {
                                    self.add_task(&TaskType::FILE, &path)?;
                                }
                            }
                        } else {
                            if self.reader.supports(&path)? {
                                info!("文件未索引，添加任务。文件: {}", path.display());
                                self.add_task(&TaskType::FILE, &path)?;
                            }
                        }
                    } else if path.is_dir() {
                        self.submit_index_all_files(&path)?;
                    }
                }
            } else if path.is_file() {
                self.indexer.delete_file(&path)?;
                if self.reader.supports(path)? {
                    info!("添加文件索引任务。文件: {}", path.display());
                    self.add_task(&TaskType::FILE, path)?;
                }
            }
        } else {
            info!("尝试删除目录或文件: {}", path.display());
            self.indexer.delete_directory(path)?;
            self.indexer.delete_file(path)?;
        }
        Ok(())
    }

    pub fn get_tasks_status(&self) -> Result<TaskStatusStat> {
        let conn = get_conn()?;
        let (pending, running, failed) = conn.query_one("SELECT COUNT(if(status = ?1, 1, NULL)), COUNT(if(status = ?2, 1, NULL)), COUNT(if(status = ?3, 1, NULL)) FROM tasks", 
            params![TaskStatus::PENDING.to_string(), TaskStatus::RUNNING.to_string(), TaskStatus::FAILED.to_string()], 
            |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?))
            }).unwrap_or((0, 0, 0));

        let mut stmt = conn.prepare("SELECT path FROM tasks WHERE status = ?1")?;
        let paths = stmt.query_map(params![TaskStatus::RUNNING.to_string()], |row| {
            Ok(row.get::<_, String>(0)?)
        })?;
        let mut running_tasks = Vec::new();
        for path in paths {
            running_tasks.push(path?);
        }

        let paths = stmt.query_map(params![TaskStatus::FAILED.to_string()], |row| {
            Ok(row.get::<_, String>(0)?)
        })?;
        let mut failed_tasks = Vec::new();
        for path in paths {
            failed_tasks.push(path?);
        }
        Ok(TaskStatusStat {
            pending,
            running,
            failed,
            running_tasks,
            failed_tasks,
        })
    }

    pub fn start_process() -> Result<()> {
        let num_cpus = std::thread::available_parallelism().map_or(1, |n| n.get());
        let num_threads = std::cmp::max(1, num_cpus / 2);
        info!("启动 {} 索引线程", num_threads);
        for i in 0..num_threads {
            thread::Builder::new()
                .name(format!("index-worker-thread-{}", i))
                .spawn(move || {
                    let worker = Worker::new().unwrap();
                    loop {
                        match worker.process_task() {
                            Ok(_) => {}
                            Err(e) => {
                                error!("处理任务失败: {}", e);
                                error!("{}", e.backtrace());
                            }
                        }
                    }
                }).unwrap();
        }
        Ok(())
    }

    pub fn process_task(&self) -> Result<()> {
        let conn = get_conn()?;
        let task = conn.query_row(
            r"UPDATE tasks
            SET status = ?1, updated_at = ?2
            WHERE id = (
                SELECT id FROM tasks 
                WHERE status = ?3
                ORDER BY id
                LIMIT 1
            )
            RETURNING id, type, path",
            params![
                TaskStatus::RUNNING.to_string(),
                Local::now().to_rfc3339(),
                TaskStatus::PENDING.to_string()
            ],
            |row| {
                let id = row.get::<_, i64>(0)?;
                let task_type = row.get::<_, String>(1)?;
                let path = row.get::<_, String>(2)?;
                Ok((id, task_type, path))
            },
        );

        match task {
            Ok((id, task_type, path)) => {
                debug!("处理任务: {:?}", (&id, &task_type, &path));
                let path = Path::new(&path);
                let task_type = TaskType::from_str(&task_type).unwrap();
                match task_type {
                    TaskType::DIRECTORY => {
                        if path.is_dir() {
                            self.indexer.write_directory(path)?;
                        }
                    }
                    TaskType::FILE => {
                        if path.is_file() {
                            match self.reader.read(path) {
                                Ok(items) => {
                                    self.indexer.write_file_items(path, items)?;
                                },
                                Err(e) => {
                                    error!("读取文件失败: {} 错误: {}", path.display(), e);
                                }
                            }
                        }
                    }
                }
                conn.execute("delete from tasks where id = ?", params![id])?;
            }
            Err(rusqlite::Error::QueryReturnedNoRows) => {
                // 没有待处理的任务，休息1s
                thread::sleep(Duration::from_secs(1));
                return Ok(());
            }
            Err(e) => {
                return Err(anyhow!("Failed to process task: {}", e));
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::fs::{self, rename};
    use fs_extra::dir::{copy, CopyOptions};
    use fs_extra::file::write_all;

    use super::*;
    use crate::test::test::TestEnv;
    use crate::worker::Worker;
    use crate::indexer::Indexer;

    
    #[test]
    fn test_add_task() {
        let (_env, temp_test_data_worker) = prepare_test_data_worker();
        let worker = Worker::new().unwrap();

        let task_type = TaskType::DIRECTORY;
        let path = temp_test_data_worker.join("office");

        let id = worker.add_task(&task_type, &path).unwrap();
        let id2 = worker.add_task(&task_type, &path).unwrap();
        assert_eq!(id, id2);
    }

    fn prepare_test_data_worker() -> (TestEnv, PathBuf) {
        let env = TestEnv::new();
        let worker = Worker::new().unwrap();
        let indexer = Indexer::new().unwrap();

        let source_dir = Path::new("../test_data/indexer/");
        let dest_dir = Path::new(env.temp_dir.path());
        fs::create_dir_all(&dest_dir).unwrap();
        let options = CopyOptions::new();
        copy(&source_dir, &dest_dir, &options).unwrap();

        let temp_test_data_worker = dest_dir.join("indexer");

        worker
            .submit_index_all_files(&temp_test_data_worker)
            .unwrap();

        let worker_status = worker.get_tasks_status().unwrap();
        assert_eq!(worker_status.pending, 4);

        for _ in 0..4 {
            worker.process_task().unwrap();
        }

        let worker_status = worker.get_tasks_status().unwrap();
        assert_eq!(worker_status.pending, 0);
        let indexer_status = indexer.get_index_status().unwrap();
        assert_eq!(indexer_status.directories, 2);
        assert_eq!(indexer_status.files, 2);

        (env, temp_test_data_worker)
    }

    #[test]
    fn test_index_all_files() {
        let _ = prepare_test_data_worker();
    }

    #[test]
    fn test_index_all_files_delete_file() {
        let (_env, temp_test_data_worker) = prepare_test_data_worker();
        let worker = Worker::new().unwrap();
        let indexer = Indexer::new().unwrap();

        fs::remove_file(temp_test_data_worker.join("office").join("test.docx")).unwrap();
        worker
            .submit_index_all_files(&temp_test_data_worker)
            .unwrap();
        let worker_status = worker.get_tasks_status().unwrap();
        assert_eq!(worker_status.pending, 1);

        let indexer_status = indexer.get_index_status().unwrap();
        assert_eq!(indexer_status.directories, 2);
        assert_eq!(indexer_status.files, 1);
    }

    #[test]
    fn test_index_all_files_delete_directory() {
        let (_env, temp_test_data_worker) = prepare_test_data_worker();
        let worker = Worker::new().unwrap();
        let indexer = Indexer::new().unwrap();

        fs::remove_dir_all(temp_test_data_worker.join("office")).unwrap();
        worker
            .submit_index_all_files(&temp_test_data_worker)
            .unwrap();
        let worker_status = worker.get_tasks_status().unwrap();
        assert_eq!(worker_status.pending, 1);

        let indexer_status = indexer.get_index_status().unwrap();
        assert_eq!(indexer_status.directories, 1);
        assert_eq!(indexer_status.files, 1);
    }

    
    #[test]
    fn test_index_all_files_add_file() {
        let (_env, temp_test_data_worker) = prepare_test_data_worker();
        let worker = Worker::new().unwrap();
        let indexer = Indexer::new().unwrap();

        write_all(temp_test_data_worker.join("test_index_all_files_add_file.txt"), "contents" ).unwrap();
        worker
            .submit_index_all_files(&temp_test_data_worker)
            .unwrap();
        let worker_status = worker.get_tasks_status().unwrap();
        assert_eq!(worker_status.pending, 2);

        for _ in 0..2 {
            worker.process_task().unwrap();
        }

        let indexer_status = indexer.get_index_status().unwrap();
        assert_eq!(indexer_status.directories, 2);
        assert_eq!(indexer_status.files, 3);
    }

    #[test]
    fn test_index_all_files_add_directory() {
        let (_env, temp_test_data_worker) = prepare_test_data_worker();
        let worker = Worker::new().unwrap();
        let indexer = Indexer::new().unwrap();

        fs::create_dir_all(temp_test_data_worker.join("new_dir")).unwrap();
        worker
            .submit_index_all_files(&temp_test_data_worker)
            .unwrap();
        let worker_status = worker.get_tasks_status().unwrap();
        assert_eq!(worker_status.pending, 2);

        for _ in 0..2 {
            worker.process_task().unwrap();
        }

        let indexer_status = indexer.get_index_status().unwrap();
        assert_eq!(indexer_status.directories, 3);
        assert_eq!(indexer_status.files, 2);
    }

    #[test]
    fn test_index_all_files_add_directory_and_file() {
        let (_env, temp_test_data_worker) = prepare_test_data_worker();
        let worker = Worker::new().unwrap();
        let indexer = Indexer::new().unwrap();

        fs::create_dir_all(temp_test_data_worker.join("new_dir")).unwrap();
        write_all(temp_test_data_worker.join("new_dir").join("test_index_all_files_add_file.txt"), "contents" ).unwrap();
        worker
            .submit_index_all_files(&temp_test_data_worker)
            .unwrap();
        let worker_status = worker.get_tasks_status().unwrap();
        assert_eq!(worker_status.pending, 3);

        for _ in 0..3 {
            worker.process_task().unwrap();
        }

        let indexer_status = indexer.get_index_status().unwrap();
        assert_eq!(indexer_status.directories, 3);
        assert_eq!(indexer_status.files, 3);
    }

    #[test]
    fn test_index_all_files_mod_file() {
        let (_env, temp_test_data_worker) = prepare_test_data_worker();
        let worker = Worker::new().unwrap();
        let indexer = Indexer::new().unwrap();

        write_all(temp_test_data_worker.join("1.txt"), "contents" ).unwrap();
        worker
            .submit_index_all_files(&temp_test_data_worker)
            .unwrap();
        let worker_status = worker.get_tasks_status().unwrap();
        assert_eq!(worker_status.pending, 1);

        worker.process_task().unwrap();

        let indexer_status = indexer.get_index_status().unwrap();
        assert_eq!(indexer_status.directories, 2);
        assert_eq!(indexer_status.files, 2);
    }

    #[test]
    fn test_index_all_files_mod_directory() {
        let (_env, temp_test_data_worker) = prepare_test_data_worker();
        let worker = Worker::new().unwrap();
        let indexer = Indexer::new().unwrap();

        rename(temp_test_data_worker.join("office"), temp_test_data_worker.join("new_office")).unwrap();
        worker
            .submit_index_all_files(&temp_test_data_worker)
            .unwrap();
        let worker_status = worker.get_tasks_status().unwrap();
        assert_eq!(worker_status.pending, 3);

        for _ in 0..3 {
            worker.process_task().unwrap();
        }

        let indexer_status = indexer.get_index_status().unwrap();
        assert_eq!(indexer_status.directories, 2);
        assert_eq!(indexer_status.files, 2);
    }

    #[test]
    fn test_get_tasks_status() {
        let _env = TestEnv::new();
        let worker = Worker::new().unwrap();
        worker
            .submit_index_all_files(Path::new("../test_data/indexer"))
            .unwrap();

        let status = worker.get_tasks_status().unwrap();
        assert_eq!(status.pending, 4);
        assert_eq!(status.running, 0);
        assert_eq!(status.failed, 0);
        assert_eq!(status.running_tasks, Vec::<String>::new());
        assert_eq!(status.failed_tasks, Vec::<String>::new());
    }

    #[test]
    fn test_process_task() {
        let _env = TestEnv::new();
        let worker = Worker::new().unwrap();
        worker
            .submit_index_all_files(&Path::new("../test_data/indexer").canonicalize().unwrap())
            .unwrap();

        let status = worker.get_tasks_status().unwrap();
        assert_eq!(status.pending, 4);
        assert_eq!(status.running, 0);
        assert_eq!(status.failed, 0);
        assert_eq!(status.running_tasks, Vec::<String>::new());
        assert_eq!(status.failed_tasks, Vec::<String>::new());

        let _ = worker.process_task().unwrap();
        let status = worker.get_tasks_status().unwrap();
        assert_eq!(status.pending, 3);
        assert_eq!(status.running, 0);
        assert_eq!(status.failed, 0);
        assert_eq!(status.running_tasks, Vec::<String>::new());
        assert_eq!(status.failed_tasks, Vec::<String>::new());

        for _ in 0..3 {
            let _ = worker.process_task().unwrap();
        }
        let status = worker.get_tasks_status().unwrap();
        assert_eq!(status.pending, 0);
        assert_eq!(status.running, 0);
        assert_eq!(status.failed, 0);
        assert_eq!(status.running_tasks, Vec::<String>::new());
        assert_eq!(status.failed_tasks, Vec::<String>::new());

        let _ = worker.process_task();
        let status = worker.get_tasks_status().unwrap();
        assert_eq!(status.pending, 0);
        assert_eq!(status.running, 0);
        assert_eq!(status.failed, 0);
        assert_eq!(status.running_tasks, Vec::<String>::new());
        assert_eq!(status.failed_tasks, Vec::<String>::new());
    }
}
