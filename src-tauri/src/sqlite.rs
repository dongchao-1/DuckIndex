use std::{fs::create_dir_all, path::PathBuf};

use once_cell::sync::OnceCell;
use r2d2::{Pool, PooledConnection};
use r2d2_sqlite::SqliteConnectionManager;
use anyhow::Result;

use crate::CONFIG;

// 全局静态变量
static POOL: OnceCell<Pool<SqliteConnectionManager>> = OnceCell::new();

pub fn init_pool() {
    let sqlite_path = PathBuf::from(CONFIG.get().unwrap().index_path.clone());
    if !sqlite_path.exists() {
        create_dir_all(&sqlite_path).unwrap();
    }

    let manager = SqliteConnectionManager::file(sqlite_path.join("index.db")).with_init(|conn| {
            // 启用 auto_vacuum (1 是 FULL 模式)
            conn.execute_batch(
                "PRAGMA auto_vacuum = FULL; \
                 PRAGMA journal_mode = WAL;"
            )?;
            Ok(())
        });
    let pool = Pool::new(manager).expect("Failed to create pool");
    POOL.set(pool).expect("Pool already initialized");
}

pub fn get_pool() -> Result<PooledConnection<SqliteConnectionManager>> {
    Ok(POOL.get().expect("Pool not initialized").get()?)
}
