use anyhow::Result;
use once_cell::sync::OnceCell;
use r2d2::{Pool, PooledConnection};
use r2d2_sqlite::SqliteConnectionManager;

use crate::dirs::get_index_dir;

// 全局静态变量
static POOL: OnceCell<Pool<SqliteConnectionManager>> = OnceCell::new();

pub fn init_pool() {
    POOL.get_or_init(|| {
        let sqlite_path = get_index_dir();

        let manager = SqliteConnectionManager::file(sqlite_path.join("index.db")).with_init(|conn| {
            // 启用 auto_vacuum (1 是 FULL 模式)
            conn.execute_batch(
                "PRAGMA auto_vacuum = FULL; \
                    PRAGMA journal_mode = WAL;",
            )?;
            Ok(())
        });
        Pool::new(manager).expect("Failed to create pool")
    });
}

pub fn get_conn() -> Result<PooledConnection<SqliteConnectionManager>> {
    Ok(POOL.get().expect("Pool not initialized").get()?)
}
