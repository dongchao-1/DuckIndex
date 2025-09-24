use std::sync::{Arc, Mutex};

use anyhow::{anyhow, Context, Result};
use log::{error, info};
use once_cell::sync::OnceCell;
use r2d2::{Pool, PooledConnection};
use duckdb::DuckdbConnectionManager;

use crate::dirs::get_index_dir;

// 全局静态变量
static POOL: OnceCell<Arc<Mutex<Option<Pool<DuckdbConnectionManager>>>>> = OnceCell::new();

pub fn init_pool() {
    POOL.get_or_init(|| {
        info!("初始化连接池...");
        let sqlite_path = get_index_dir().join("index.db");

        let manager = DuckdbConnectionManager::file(sqlite_path).expect("Failed to create DuckDB manager");
        Arc::new(Mutex::new(Some(
            Pool::new(manager).expect("Failed to create pool"),
        )))
    });
}

pub fn get_conn() -> Result<PooledConnection<DuckdbConnectionManager>> {
    Ok(POOL
        .get()
        .expect("Pool not initialized")
        .lock()
        .map_err(|e| {
            error!("获取数据库连接失败: {e:?}");
            anyhow::anyhow!("获取数据库连接失败")
        })?
        .as_ref()
        .context("获取数据库连接as_ref失败")?
        .get()?)
}

pub fn close_pool() {
    info!("关闭连接池...");
    let conn = get_conn().expect("Failed to get connection");
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
        let conn = get_conn()?;
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
    let conn = get_conn()?;
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
