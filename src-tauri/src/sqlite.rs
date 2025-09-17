use std::sync::{Arc, Mutex};

use anyhow::{anyhow, Context, Result};
use log::{error, info};
use once_cell::sync::OnceCell;
use r2d2::{Pool, PooledConnection};
use r2d2_sqlite::SqliteConnectionManager;

use crate::dirs::get_index_dir;

// 全局静态变量
static POOL: OnceCell<Arc<Mutex<Option<Pool<SqliteConnectionManager>>>>> = OnceCell::new();

pub fn init_pool() {
    POOL.get_or_init(|| {
        info!("初始化连接池...");
        let sqlite_path = get_index_dir().join("index.db");

        let manager = SqliteConnectionManager::file(sqlite_path).with_init(|conn| {
            conn.execute_batch(r"PRAGMA busy_timeout = 2147483647;")?;

            conn.busy_handler(Some(|_retries| true))?;

            Ok(())
        });
        Arc::new(Mutex::new(Some(
            Pool::new(manager).expect("Failed to create pool"),
        )))
    });
}

pub fn get_conn() -> Result<PooledConnection<SqliteConnectionManager>> {
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
    conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE); vacuum;")
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
            r#"PRAGMA journal_mode = WAL;
            PRAGMA auto_vacuum = FULL;

            -- config.rs
            DROP TABLE IF EXISTS config;
            CREATE TABLE config (
                id INTEGER PRIMARY KEY,
                key TEXT NOT NULL,
                value TEXT NOT NULL,
                unique (key)
            );
            INSERT INTO config (key, value) VALUES ('IndexDirPaths', '[]');
            INSERT INTO config (key, value) VALUES ('ExtensionWhitelist', '[{"label":"文档","is_extension":false,"children":[{"label":"txt","is_extension":true,"enabled":true},{"label":"md","is_extension":true,"enabled":true},{"label":"markdown","is_extension":true,"enabled":true},{"label":"docx","is_extension":true,"enabled":true},{"label":"pptx","is_extension":true,"enabled":true},{"label":"pdf","is_extension":true,"enabled":true}]}, {"label":"数据","is_extension":false,"children":[{"label":"xlsx","is_extension":true,"enabled":false}]}, {"label":"图片","is_extension":false,"children":[{"label":"jpg","is_extension":true,"enabled":true},{"label":"jpeg","is_extension":true,"enabled":true},{"label":"png","is_extension":true,"enabled":true},{"label":"tif","is_extension":true,"enabled":true},{"label":"tiff","is_extension":true,"enabled":true},{"label":"gif","is_extension":true,"enabled":true},{"label":"webp","is_extension":true,"enabled":true}]}]');

            -- indexer.rs
            DROP TABLE IF EXISTS directories;
            CREATE TABLE directories (
                id INTEGER PRIMARY KEY,
                name TEXT NOT NULL,
                path TEXT NOT NULL,
                modified_time TEXT NOT NULL,
                UNIQUE (path)
            );
            CREATE INDEX idx_directories_name ON directories (name);
            DROP TABLE IF EXISTS files;
            CREATE TABLE files (
                id INTEGER PRIMARY KEY,
                directory_id INTEGER NOT NULL,
                name TEXT NOT NULL,
                modified_time TEXT NOT NULL,
                UNIQUE (directory_id, name)
            );
            CREATE INDEX idx_files_name ON files (name);
            DROP TABLE IF EXISTS items;
            CREATE TABLE items (
                id INTEGER PRIMARY KEY,
                file_id INTEGER NOT NULL,
                content TEXT NOT NULL
            );
            CREATE INDEX idx_items_file_id ON items (file_id);

            -- worker.rs
            DROP TABLE IF EXISTS tasks;
            CREATE TABLE tasks (
                id INTEGER PRIMARY KEY,
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
        .query_one("select version from db_version", [], |row| {
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
