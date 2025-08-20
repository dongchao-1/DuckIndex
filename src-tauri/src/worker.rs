use std::fs;
use std::path::Path;
use std::thread;
use anyhow::{anyhow, Result};
use rusqlite::params;
use strum::Display;
use strum::EnumString;
use chrono::Local;
use std::str::FromStr;
use std::time::Duration;
use serde::Serialize;
use serde_json::{Result as jsonResult, Value as jsonValue};

use crate::indexer::Indexer;
use crate::reader::CompositeReader;
use crate::sqlite::get_pool;

pub struct Worker {
    indexer: Indexer,
    reader: CompositeReader,
}

#[derive(Debug, PartialEq, EnumString, Display)]
enum TaskType {
    #[strum(serialize = "DIRECTORY")]
    DIRECTORY,
    #[strum(serialize = "FILE")]
    FILE
}

#[derive(Debug, PartialEq, EnumString, Display)]
enum TaskStatus {
    #[strum(serialize = "PENDING")]
    PENDING,
    #[strum(serialize = "RUNNING")]
    RUNNING,
    #[strum(serialize = "FAILED")]
    FAILED
}

#[derive(Debug, Serialize)]
pub struct TaskStatusStat {
    pending: usize,
    running: usize,
    failed: usize,
    running_tasks: Vec<String>,
    failed_tasks: Vec<String>,
}

impl std::fmt::Display for TaskStatusStat {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        let json_string = serde_json::to_string_pretty(&self).unwrap();
        write!(f, "{}", json_string)
    }
}

impl Worker {

    pub fn check_or_init() -> Result<()> {
        if let Err(_) = Self::check_worker_init() {
            Self::reset_worker()?;
        }
        Ok(())
    }
    
    fn check_worker_init() -> Result<()> {
        let conn = get_pool()?;
        let row = conn.query_one("select version from worker_version", [], |row|
            row.get::<_, String>(0)
        ).map_err(|e| anyhow!("Worker not initialized: {}", e))?;

        if row != "0.1" {
            return Err(anyhow!("Worker version mismatch: expected 0.1, found {}", row));
        }
        Ok(())
    }

    fn reset_worker() -> Result<()> {
        let conn = get_pool()?;
        conn.execute_batch(
            r"
            DROP TABLE IF EXISTS tasks;
            CREATE TABLE tasks (
                id INTEGER PRIMARY KEY,
                type TEXT NOT NULL,
                path TEXT NOT NULL,
                status TEXT NOT NULL,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );
            DROP TABLE IF EXISTS worker_version;
            CREATE TABLE worker_version (
                version TEXT
            );
            INSERT INTO worker_version (version) VALUES ('0.1');
        ")?;
        Ok(())
    }

    pub fn reset_running_tasks() -> Result<()> {
        let conn = get_pool()?;
        // TODO 应该不需要清理
        // let mut stmt = conn.prepare("select type, path from tasks where status = ?1")?;
        // let rows = stmt.query_map(params![TaskStatus::RUNNING.to_string()], |row| {
        //     let task_type: String = row.get(0)?;
        //     let path: String = row.get(1)?;
        //     Ok((task_type, path))
        // })?;
        // let indexer = Indexer::new()?;
        // for row in rows {
        //     let (task_type, path) = row?;
        //     match TaskType::from_str(&task_type)? {
        //         TaskType::DIRECTORY => {
        //             indexer.delete_directory(Path::new(&path))?;
        //         }
        //         TaskType::FILE => {
        //             indexer.delete_file(Path::new(&path))?;
        //         }
        //     }
        // }

        conn.execute(
            "UPDATE tasks SET status = ?1, updated_at = ?2 WHERE status = ?3",
            params![TaskStatus::PENDING.to_string(), Local::now().to_rfc3339(), TaskStatus::RUNNING.to_string()]
        )?;
        Ok(())
    }

    pub fn new() -> Result<Worker> {
        let indexer = Indexer::new()?;
        let reader = CompositeReader::new()?;
        Ok(Worker { indexer, reader })
    }

    fn add_task(&self, task_type: &TaskType, path: &Path) -> Result<()> {
        let conn = get_pool()?;

        let path = path.canonicalize()?.to_str().unwrap().to_string();
        let now = Local::now().to_rfc3339();
        conn.execute(
            "INSERT INTO tasks (type, path, status, created_at, updated_at) VALUES (?, ?, ?, ?, ?)",
            params![task_type.to_string(), path, TaskStatus::PENDING.to_string(), now, now],
        )?;
        Ok(())
    }

    pub fn submit_index_all_files(&self, path: &Path) -> Result<()> {
        if path.is_dir() {
            self.add_task(&TaskType::DIRECTORY, path)?;
            for entry in fs::read_dir(path)? {
                let entry = entry?;
                let path = entry.path();
                if path.is_file() {
                    self.add_task(&TaskType::FILE, &path)?;
                } else if path.is_dir() {
                    self.submit_index_all_files(&path)?;
                }
            }
        } else {
            eprintln!("Path does not exist: {}", path.display());
        }
        Ok(())
    }

    pub fn get_tasks_status(&self) -> Result<TaskStatusStat> {
        let conn = get_pool()?;
        let (pending, running, failed) = conn.query_one("SELECT COUNT(if(status = ?1, 1, NULL)), COUNT(if(status = ?2, 1, NULL)), COUNT(if(status = ?3, 1, NULL)) FROM tasks", 
            params![TaskStatus::PENDING.to_string(), TaskStatus::RUNNING.to_string(), TaskStatus::FAILED.to_string()], 
            |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?))
            }).unwrap_or((0, 0, 0));
        
        let mut stmt = conn.prepare("SELECT path FROM tasks WHERE status = ?1")?;
        let paths = stmt.query_map(
            params![TaskStatus::RUNNING.to_string()],
            |row| {
                Ok(row.get::<_, String>(0)?)
            }
        )?;
        let mut running_tasks = Vec::new();
        for path in paths {
            running_tasks.push(path?);
        }

        let paths = stmt.query_map(
            params![TaskStatus::FAILED.to_string()],
            |row| {
                Ok(row.get::<_, String>(0)?)
            }
        )?;
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
        let num_cpus = std::thread::available_parallelism()
            .map_or(1, |n| n.get());
        let num_threads = std::cmp::max(1, num_cpus / 2);
        println!("Starting {} process threads", num_threads);
        for _ in 0..num_threads {
            std::thread::spawn(move || {
                let worker = Worker::new().unwrap();
                loop {
                    match worker.process_task() {
                        Ok(_) => {}
                        Err(e) => {
                            // TODO 失败处理
                            eprintln!("Error processing task: {}", e);
                        }
                    }
                }
            });
        }
        Ok(())
    }

    fn process_task(&self) -> Result<()> {
        let conn = get_pool()?;
        let task = conn.query_row(r"UPDATE tasks
            SET status = ?1, updated_at = ?2
            WHERE id = (
                SELECT id FROM tasks 
                WHERE status = ?3
                ORDER BY id
                LIMIT 1
            )
            RETURNING id, type, path", 
            params![TaskStatus::RUNNING.to_string(), Local::now().to_rfc3339(), TaskStatus::PENDING.to_string()], 
            |row| {
                let id = row.get::<_, i64>(0)?;
                let task_type = row.get::<_, String>(1)?;
                let path = row.get::<_, String>(2)?;
                Ok((id, task_type, path))
            }
        );

        match task {
            Ok((id, task_type, path)) => {               
                let path = Path::new(&path);
                let task_type = TaskType::from_str(&task_type).unwrap();
                match task_type {
                    TaskType::DIRECTORY => {
                        // 处理目录任务
                        self.indexer.write_directory(path)?;
                    }
                    TaskType::FILE => {
                        // 处理文件任务
                        if let Ok(items) = self.reader.read(path) {
                            self.indexer.write_file_items(path, items)?;
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
    use super::*;
    use crate::{test::test::TestEnv};

    #[test]
    fn test_index_all_files() {
        let _env = TestEnv::new();
        Worker::reset_worker().unwrap();
        let worker = Worker::new().unwrap();
        worker.submit_index_all_files(Path::new("../test_data")).unwrap();
    }

    
    #[test]
    fn test_get_tasks_status() {
        let _env = TestEnv::new();
        Worker::reset_worker().unwrap();
        let worker = Worker::new().unwrap();
        worker.submit_index_all_files(Path::new("../test_data")).unwrap();

        let status = worker.get_tasks_status().unwrap();
        assert_eq!(status.pending, 6);
        assert_eq!(status.running, 0);
        assert_eq!(status.failed, 0);
        assert_eq!(status.running_tasks, Vec::<String>::new());
        assert_eq!(status.failed_tasks, Vec::<String>::new());
    }

    #[test]
    fn test_process_task() {
        let _env = TestEnv::new();
        Worker::reset_worker().unwrap();
        let worker = Worker::new().unwrap();
        worker.submit_index_all_files(Path::new("../test_data")).unwrap();

        let _ = worker.process_task().unwrap();
        let status = worker.get_tasks_status().unwrap();
        assert_eq!(status.pending, 5);
        assert_eq!(status.running, 0);
        assert_eq!(status.failed, 0);
        assert_eq!(status.running_tasks, Vec::<String>::new());
        assert_eq!(status.failed_tasks, Vec::<String>::new());

        for _ in 0..5 {
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
