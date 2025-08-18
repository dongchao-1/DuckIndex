use std::fs;
use std::path::Path;
use anyhow::{anyhow, Result};
use r2d2::PooledConnection;
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::params;
use strum::Display;
use strum::EnumString;
use chrono::Local;
use std::str::FromStr;

use crate::indexer::Indexer;
use crate::sqlite::get_pool;

struct Worker {
    conn: PooledConnection<SqliteConnectionManager>,
    indexer: Indexer,
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
    #[strum(serialize = "COMPLETED")]
    COMPLETED,
    #[strum(serialize = "FAILED")]
    FAILED
}

struct TaskStatusStat {
    pending: usize,
    running: usize,
    completed: usize,
    failed: usize,
}

impl Worker {

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

    pub fn new() -> Result<Worker> {
        if let Err(_) = Self::check_worker_init() {
            Self::reset_worker()?;
        }
        // println!("Opening index at: {:?}", index_path);
        let conn = get_pool()?;
        let indexer = Indexer::new()?;
        Ok(Worker { conn, indexer })
    }

    fn add_task(&self, taks_type: &TaskType, path: &Path) -> Result<()> {
        let conn = get_pool()?;

        let path = path.canonicalize()?.to_str().unwrap().to_string();
        let now = Local::now().to_rfc3339();
        conn.execute(
            "INSERT INTO tasks (type, path, status, created_at, updated_at) VALUES (?, ?, ?, ?, ?)",
            params![taks_type.to_string(), path, TaskStatus::PENDING.to_string(), now, now],
        )?;
        Ok(())
    }

    pub fn index_all_files(&self, path: &Path) -> Result<()> {
        if path.is_dir() {
            self.add_task(&TaskType::DIRECTORY, path)?;
            for entry in fs::read_dir(path)? {
                let entry = entry?;
                let path = entry.path();
                if path.is_file() {
                    self.add_task(&TaskType::FILE, &path)?;
                } else if path.is_dir() {
                    self.index_all_files(&path)?;
                }
            }
        } else {
            eprintln!("Path does not exist: {}", path.display());
        }
        Ok(())
    }

    fn get_tasks_status(&self) -> Result<TaskStatusStat> {
        let row = self.conn.query_one("SELECT COUNT(if(status = ?1, 1, NULL)), COUNT(if(status = ?2, 1, NULL)), COUNT(if(status = ?3, 1, NULL)), COUNT(if(status = ?4, 1, NULL)) FROM tasks", 
            params![TaskStatus::PENDING.to_string(), TaskStatus::RUNNING.to_string(), TaskStatus::COMPLETED.to_string(), TaskStatus::FAILED.to_string()], 
            |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
            }).unwrap_or((0, 0, 0, 0));
        Ok(TaskStatusStat {
            pending: row.0,
            running: row.1,
            completed: row.2,
            failed: row.3,
        })
    }

    fn process_task(&self) -> ! {
        loop {
            let task = self.conn.query_row("SELECT id, type, path FROM tasks WHERE status = ?1 LIMIT 1", 
                params![TaskStatus::PENDING.to_string()], 
                |row| {
                    Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?, row.get::<_, String>(2)?))
                }).ok();

            if let Some((id, task_type, path)) = task {
                self.conn.execute("UPDATE tasks SET status = ?1 WHERE id = ?2", params![TaskStatus::RUNNING.to_string(), id]).unwrap();
                
                match TaskType::from_str(&task_type) {
                    Ok(TaskType::DIRECTORY) => {
                        // 处理目录任务
                    }
                    Ok(TaskType::FILE) => {
                        // 处理文件任务
                    }
                    Err(_) => todo!(),
                }
            }
        }
    }

    // fn index_dir(&self, path:&Path, reader: &reader::CompositeReader, indexer: &Indexer, tx: &Sender<String>, dir_stat: &mut DirStatistics) -> Result<(), Box<dyn std::error::Error>> {
    //     // println!("Indexing directory: {}", path.display());
    //     // tx.send(format!("Indexing directory: {}", path.display())).unwrap();
    //     indexer.write_directory(path).unwrap();
    //     let entries = fs::read_dir(path)?;
    //     for entry in entries {
    //         let entry = entry?;
    //         let file_type = entry.file_type().unwrap();
    //         if file_type.is_file() {
    //             // 处理文件
    //             // println!("Indexing file: {}", entry.path().display());
    //             tx.send(format!("正在索引 {}, 总文件数: {}, 已完成: {}, 当前文件: {}", dir_stat.dir_path, dir_stat.file_count, dir_stat.index_count, entry.path().display())).unwrap();
    //             let file = entry.path();
    //             if let Ok(items) = reader.read(&file) {
    //                 indexer.write_file_items(&file, items).unwrap();
    //             }
    //             dir_stat.index_count += 1;
    //             // if dir_stat.index_count % 100 == 0 || dir_stat.index_count == dir_stat.file_count {
    //             //     tx.send(format!("正在索引 {}, 总文件数: {}, 已完成: {}", dir_stat.dir_path, dir_stat.file_count, dir_stat.index_count)).unwrap();
    //             // }
    //         } else if file_type.is_dir() {
    //             // 递归处理子目录
    //             index_dir(&entry.path(), reader, indexer, tx, dir_stat).unwrap();
    //         }
    //     }
    //     Ok(())
    // }

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
        worker.index_all_files(Path::new("../test_data")).unwrap();
    }

    
    #[test]
    fn test_get_tasks_status() {
        let _env = TestEnv::new();
        Worker::reset_worker().unwrap();
        let worker = Worker::new().unwrap();
        worker.index_all_files(Path::new("../test_data")).unwrap();

        let status = worker.get_tasks_status().unwrap();
        assert_eq!(status.pending, 7);
        assert_eq!(status.running, 0);
        assert_eq!(status.completed, 0);
        assert_eq!(status.failed, 0);
    }
}
