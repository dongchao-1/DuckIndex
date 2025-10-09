use std::sync::{Arc, Mutex, RwLock, RwLockWriteGuard};
use std::collections::HashMap;

use anyhow::{anyhow, Context, Result};
use log::{debug, error, info};
use once_cell::sync::OnceCell;
use r2d2::{Pool, PooledConnection};
use duckdb::{DuckdbConnectionManager, Params, Row, Transaction};
use serde::de;

use crate::dirs::get_index_dir;

/// 持有写锁和数据库连接的包装结构体
#[derive(Debug)]
pub struct WriteConnection {
    _table_locks: Vec<RwLockWriteGuard<'static, String>>, // Keep the locks alive
    conn: PooledConnection<DuckdbConnectionManager>,
}

impl WriteConnection {
    pub fn query_row<T, P, F>(&self, sql: &str, params: P, f: F) -> duckdb::Result<T>
    where
        P: Params,
        F: FnOnce(&Row<'_>) -> duckdb::Result<T>,
    {
        match self.conn.prepare(sql)?.query_row(params, f) {
            Ok(result) => Ok(result),
            Err(e) => {
                if let duckdb::Error::DuckDBFailure(_, desc) = &e {
                    if let Some(d) = desc {
                        if d.contains("write-write conflict on key") {
                            error!("write-write conflict: {}", e);
                            // TODO 重试
                        }
                    }
                    Err(e)
                } else {
                    Err(e)
                }
            }
        }
    }

    pub fn execute<P: Params>(&self, sql: &str, params: P) -> duckdb::Result<usize> {
        self.conn.execute(sql, params)
    }

    pub fn execute_batch(&self, sql: &str) -> duckdb::Result<()> {
        self.conn.execute_batch(sql)
    }

    pub fn transaction(&mut self) -> duckdb::Result<Transaction<'_>> {
        self.conn.transaction()
    }
}

// impl std::ops::Deref for WriteConnection {
//     type Target = PooledConnection<DuckdbConnectionManager>;
    
//     fn deref(&self) -> &Self::Target {
//         &self.conn
//     }
// }

// impl std::ops::DerefMut for WriteConnection {
//     fn deref_mut(&mut self) -> &mut Self::Target {
//         &mut self.conn
//     }
// }

// 全局静态变量
static POOL: OnceCell<Arc<Mutex<Option<Pool<DuckdbConnectionManager>>>>> = OnceCell::new();
static TABLE_LOCKS: OnceCell<Arc<Mutex<HashMap<String, Arc<RwLock<String>>>>>> = OnceCell::new();

pub fn init_pool() {
    POOL.get_or_init(|| {
        info!("初始化连接池...");
        let index_path = get_index_dir().join("index.db");

        let manager = DuckdbConnectionManager::file(index_path).expect("Failed to create DuckDB manager");
        Arc::new(Mutex::new(Some(
            Pool::new(manager).expect("Failed to create pool"),
            // Pool::builder().error_handler(Box::new(RetryErrorHandler::new())).max_size(15).build(manager).expect("Failed to create pool"),
        )))
    });
    
    // 初始化表锁映射
    TABLE_LOCKS.get_or_init(|| {
        Arc::new(Mutex::new(HashMap::new()))
    });
}

pub fn get_read_conn() -> Result<PooledConnection<DuckdbConnectionManager>> {
    // 读操作不需要锁，直接获取连接
    let conn = POOL
        .get()
        .expect("Pool not initialized")
        .lock()
        .map_err(|e| {
            error!("获取数据库连接失败: {e:?}");
            anyhow::anyhow!("获取数据库连接失败")
        })?
        .as_ref()
        .context("获取数据库连接as_ref失败")?
        .get()?;
    
    Ok(conn)
}

/// 获取多表写连接，按字母顺序锁定多个表以避免死锁，返回单个连接
pub fn get_multi_write_conn(table_names: &[&str]) -> Result<WriteConnection> {
    if table_names.is_empty() {
        return Err(anyhow!("至少需要指定一个表名"));
    }
    debug!("请求多表写连接，表名: {:?}", table_names);

    // 按字母顺序排序表名以避免死锁
    let mut sorted_names: Vec<&str> = table_names.to_vec();
    sorted_names.sort();
    sorted_names.dedup(); // 去重
    
    // 首先收集所有的 Arc<RwLock<()>> 并持有它们
    let mut table_locks = Vec::new();
    
    for table_name in &sorted_names {
        let table_lock_arc = {
            let mut table_locks_map = TABLE_LOCKS
                .get()
                .expect("Table locks not initialized")
                .lock()
                .map_err(|e| {
                    error!("获取表锁映射失败: {e:?}");
                    anyhow::anyhow!("获取表锁映射失败")
                })?;
            
            // 如果表锁不存在，则创建新的锁
            table_locks_map.entry(table_name.to_string())
                .or_insert_with(|| Arc::new(RwLock::new(table_name.to_string())))
                .clone()
        };
        
        let write_guard = table_lock_arc
            .write()
            .map_err(|e| {
                error!("获取表 {} 的写锁失败: {e:?}", table_name);
                anyhow::anyhow!("获取表 {} 的写锁失败", table_name)
            })?;
        
        // Use unsafe to transmute the lifetime to 'static
        // This is safe because we know the Arc<RwLock<()>> lives for the entire program duration
        let static_guard: RwLockWriteGuard<'static, String> = unsafe {
            std::mem::transmute(write_guard)
        };
        
        table_locks.push(static_guard);
    }

    let ret = WriteConnection {
        _table_locks: table_locks,
        conn: get_read_conn()?,
    };

    debug!("获取写连接: {:?}", ret);

    Ok(ret)
}

/// 获取单表写连接的便捷方法
pub fn get_write_conn(table_name: &str) -> Result<WriteConnection> {
    get_multi_write_conn(&[table_name])
}
    

pub fn close_pool() {
    info!("关闭连接池...");
    let conn = get_read_conn().expect("Failed to get connection");
    conn.execute_batch("CHECKPOINT;")
        .expect("Failed to execute batch");

    if let Some(pool_arc) = POOL.get() {
        if let Ok(mut pool_option_lock) = pool_arc.lock() {
            let pool_option = pool_option_lock.take();
            if pool_option.is_some() {
                info!("数据库连接池已关闭。");
            }
        }
    }
}

pub fn check_or_init_db() -> Result<()> {
    if check_db_init().is_err() {
        let conn = get_write_conn("db_version")?;
        conn.execute_batch(
            r#"
            -- config.rs
            DROP SEQUENCE IF EXISTS config_id;
            CREATE SEQUENCE config_id START 1;
            DROP TABLE IF EXISTS config;
            CREATE TABLE config (
                id INTEGER PRIMARY KEY DEFAULT NEXTVAL('config_id'),
                key TEXT NOT NULL,
                value TEXT NOT NULL,
                unique (key)
            );
            INSERT INTO config (key, value) VALUES ('IndexDirPaths', '[]');
            INSERT INTO config (key, value) VALUES ('ExtensionWhitelist', '[{"label":"文档","is_extension":false,"children":[{"label":"txt","is_extension":true,"enabled":true},{"label":"md","is_extension":true,"enabled":true},{"label":"markdown","is_extension":true,"enabled":true},{"label":"docx","is_extension":true,"enabled":true},{"label":"pptx","is_extension":true,"enabled":true},{"label":"pdf","is_extension":true,"enabled":true}]}, {"label":"数据","is_extension":false,"children":[{"label":"xlsx","is_extension":true,"enabled":false}]}, {"label":"图片","is_extension":false,"children":[{"label":"jpg","is_extension":true,"enabled":true},{"label":"jpeg","is_extension":true,"enabled":true},{"label":"png","is_extension":true,"enabled":true},{"label":"tif","is_extension":true,"enabled":true},{"label":"tiff","is_extension":true,"enabled":true},{"label":"gif","is_extension":true,"enabled":true},{"label":"webp","is_extension":true,"enabled":true}]}]');

            -- indexer.rs
            DROP SEQUENCE IF EXISTS directories_id;
            CREATE SEQUENCE directories_id START 1;
            DROP TABLE IF EXISTS directories;
            CREATE TABLE directories (
                id INTEGER PRIMARY KEY DEFAULT NEXTVAL('directories_id'),
                name TEXT NOT NULL,
                path TEXT NOT NULL,
                modified_time TEXT NOT NULL,
                UNIQUE (path)
            );
            CREATE INDEX idx_directories_name ON directories (name);

            DROP SEQUENCE IF EXISTS files_id;
            CREATE SEQUENCE files_id START 1;
            DROP TABLE IF EXISTS files;
            CREATE TABLE files (
                id INTEGER PRIMARY KEY DEFAULT NEXTVAL('files_id'),
                directory_id INTEGER NOT NULL,
                name TEXT NOT NULL,
                modified_time TEXT NOT NULL,
                UNIQUE (directory_id, name)
            );
            CREATE INDEX idx_files_name ON files (name);

            DROP SEQUENCE IF EXISTS items_id;
            CREATE SEQUENCE items_id START 1;
            DROP TABLE IF EXISTS items;
            CREATE TABLE items (
                id INTEGER PRIMARY KEY DEFAULT NEXTVAL('items_id'),
                file_id INTEGER NOT NULL,
                content TEXT NOT NULL
            );
            CREATE INDEX idx_items_file_id ON items (file_id);

            -- worker.rs
            DROP SEQUENCE IF EXISTS tasks_id;
            CREATE SEQUENCE tasks_id START 1;
            DROP TABLE IF EXISTS tasks;
            CREATE TABLE tasks (
                id INTEGER PRIMARY KEY DEFAULT NEXTVAL('tasks_id'),
                path_type TEXT NOT NULL,
                path TEXT NOT NULL,
                task_type TEXT NOT NULL,
                status TEXT NOT NULL,
                worker TEXT,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                UNIQUE (path_type, path)
            );
            CREATE INDEX idx_tasks_status ON tasks (status);

            -- version
            DROP TABLE IF EXISTS db_version;
            CREATE TABLE db_version (
                version TEXT
            );
            INSERT INTO db_version (version) VALUES ('0.1');
            "#,
        )?;
    }
    Ok(())
}

fn check_db_init() -> Result<()> {
    let conn = get_read_conn()?;
    let row = conn
        .query_row("select version from db_version", [], |row| {
            row.get::<_, String>(0)
        })
        .map_err(|e| anyhow!("Database not initialized: {}", e))?;

    if row != "0.1" {
        return Err(anyhow!(
            "Database version mismatch: expected 0.1, found {}",
            row
        ));
    }
    Ok(())
}


#[cfg(test)]
mod tests {
    use crate::test::test_mod::TestEnv;

    #[test]
    fn test_init_logger() {
        let _env = TestEnv::new();
        crate::duckdb::init_pool();
    }
}
