use std::sync::{Mutex, MutexGuard};

use anyhow::{anyhow, Result};
use duckdb::DuckdbConnectionManager;
use log::{debug, info};
use once_cell::sync::OnceCell;
use r2d2::{Pool, PooledConnection};

use crate::dirs::get_index_dir;

use std::ops::{Deref, DerefMut};

pub struct DebuggingMutexGuard<'a, T> {
    guard: MutexGuard<'a, T>,
    name: &'static str,
}

impl<T> Drop for DebuggingMutexGuard<'_, T> {
    fn drop(&mut self) {
        debug!("释放了 Mutex {} 的锁", self.name);
    }
}

impl<T> Deref for DebuggingMutexGuard<'_, T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        &self.guard
    }
}

impl<T> DerefMut for DebuggingMutexGuard<'_, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.guard
    }
}

pub struct DebuggingMutex<T> {
    // 这个可以加优先级，优先处理关闭等任务
    mutex: Mutex<T>,
    name: &'static str,
}

impl<T> DebuggingMutex<T> {
    pub fn new(data: T, name: &'static str) -> Self {
        Self {
            mutex: Mutex::new(data),
            name,
        }
    }

    pub fn lock(&self) -> DebuggingMutexGuard<'_, T> {
        debug!("正在尝试获取 Mutex {} 的锁...", self.name);
        let guard = self.mutex.lock().unwrap();
        debug!("成功获取了 Mutex {} 的锁", self.name);
        DebuggingMutexGuard {
            guard,
            name: self.name,
        }
    }
}

static POOL: OnceCell<DebuggingMutex<Option<Pool<DuckdbConnectionManager>>>> = OnceCell::new();
static WRITE_CONN: OnceCell<DebuggingMutex<Option<PooledConnection<DuckdbConnectionManager>>>> =
    OnceCell::new();

pub fn init_pool() {
    POOL.get_or_init(|| {
        info!("初始化连接池...");
        let index_path = get_index_dir().join("index.db");
        info!("数据库路径: {:?}", index_path);

        let manager =
            DuckdbConnectionManager::file(index_path).expect("Failed to create DuckDB manager");
        DebuggingMutex::new(
            Some(Pool::new(manager).expect("Failed to create pool")),
            "DB_POOL",
        )
    });

    WRITE_CONN.get_or_init(|| {
        let conn = POOL.get().expect("Pool not initialized").lock();

        let pool_ref = conn.as_ref().expect("Database pool is not initialized");
        let pooled_conn = pool_ref.get().expect("Failed to get connection from pool");
        DebuggingMutex::new(Some(pooled_conn), "DB_WRITE_CONN")
    });
}

pub fn get_read_conn() -> Result<PooledConnection<DuckdbConnectionManager>> {
    // 读操作不需要锁，直接获取连接
    let conn = POOL
        .get()
        .expect("Pool not initialized")
        .lock()
        .as_ref()
        .ok_or_else(|| anyhow!("Database pool is not initialized"))?
        .get()?;

    Ok(conn)
}

pub fn get_write_conn(
) -> Result<DebuggingMutexGuard<'static, Option<PooledConnection<DuckdbConnectionManager>>>> {
    // TODO 这里可以按照表加锁，提升性能
    debug!("尝试获取写锁...");
    let conn_lock = WRITE_CONN.get().expect("Write connection not initialized");
    let conn = conn_lock.lock();
    debug!("获取写锁成功");
    Ok(conn)
}

pub fn close_pool() {
    info!("关闭连接池...");
    let mut conn_lock = get_write_conn().expect("Failed to get connection");
    let conn = conn_lock.take();
    if let Some(pooled_conn) = conn {
        pooled_conn
            .execute_batch("FORCE CHECKPOINT;")
            .expect("Failed to execute batch");
        info!("写连接已关闭。");
    }

    if let Some(pool_arc) = POOL.get() {
        let mut pool_option_lock = pool_arc.lock();
        let pool_option = pool_option_lock.take();
        if pool_option.is_some() {
            info!("数据库连接池已关闭。");
        }
    }
}

pub fn check_or_init_db() -> Result<()> {
    if check_db_init().is_err() {
        let mut conn = get_write_conn()?;
        let conn = conn
            .as_mut()
            .ok_or_else(|| anyhow!("Database write connection is not initialized"))?;
        let tx = conn.transaction()?;
        tx.execute_batch(
            r#"
            -- config.rs
            DROP TABLE IF EXISTS config;
            CREATE TABLE config (
                id UUID PRIMARY KEY DEFAULT uuidv7(),
                key TEXT NOT NULL,
                value TEXT NOT NULL,
                unique (key)
            );
            INSERT INTO config (key, value) VALUES ('IndexDirPaths', '[]');
            INSERT INTO config (key, value) VALUES ('ExtensionWhitelist', '[{"label":"文档","is_extension":false,"children":[{"label":"txt","is_extension":true,"enabled":true},{"label":"md","is_extension":true,"enabled":true},{"label":"markdown","is_extension":true,"enabled":true},{"label":"docx","is_extension":true,"enabled":true},{"label":"pptx","is_extension":true,"enabled":true},{"label":"pdf","is_extension":true,"enabled":true}]}, {"label":"数据","is_extension":false,"children":[{"label":"xlsx","is_extension":true,"enabled":false}]}, {"label":"图片","is_extension":false,"children":[{"label":"jpg","is_extension":true,"enabled":true},{"label":"jpeg","is_extension":true,"enabled":true},{"label":"png","is_extension":true,"enabled":true},{"label":"tif","is_extension":true,"enabled":true},{"label":"tiff","is_extension":true,"enabled":true},{"label":"gif","is_extension":true,"enabled":true},{"label":"webp","is_extension":true,"enabled":true}]}]');

            -- indexer.rs
            DROP TABLE IF EXISTS directories;
            CREATE TABLE directories (
                id UUID PRIMARY KEY DEFAULT uuidv7(),
                name TEXT NOT NULL,
                path TEXT NOT NULL,
                modified_time TEXT NOT NULL,
                UNIQUE (path)
            );
            CREATE INDEX idx_directories_name ON directories (name);

            DROP TABLE IF EXISTS files;
            CREATE TABLE files (
                id UUID PRIMARY KEY DEFAULT uuidv7(),
                directory_id UUID NOT NULL,
                name TEXT NOT NULL,
                modified_time TEXT NOT NULL,
                UNIQUE (directory_id, name)
            );
            CREATE INDEX idx_files_name ON files (name);

            DROP TABLE IF EXISTS items;
            CREATE TABLE items (
                id UUID PRIMARY KEY DEFAULT uuidv7(),
                file_id UUID NOT NULL,
                content TEXT NOT NULL
            );
            CREATE INDEX idx_items_file_id ON items (file_id);

            -- worker.rs
            DROP TABLE IF EXISTS tasks;
            CREATE TABLE tasks (
                id UUID PRIMARY KEY DEFAULT uuidv7(),
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
        tx.commit()?;
        info!("数据库初始化完成");
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
    use rusty_fork::rusty_fork_test;

    rusty_fork_test! {
        #[test]
        fn test_init_logger() {
            let _env = TestEnv::new();
            crate::duckdb::init_pool();
        }
    }
}
