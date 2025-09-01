use std::sync::{Arc, Mutex};

use anyhow::{Context, Result};
use log::info;
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
            // 启用 auto_vacuum (1 是 FULL 模式)
            conn.execute_batch(
                "PRAGMA auto_vacuum = FULL; \
                    PRAGMA journal_mode = WAL;",
            )?;
            Ok(())
        });
        Arc::new(Mutex::new(Some(Pool::new(manager).expect("Failed to create pool"))))
    });
}

pub fn get_conn() -> Result<PooledConnection<SqliteConnectionManager>> {
    Ok(POOL.get().expect("Pool not initialized").lock().map_err(|e| {
        log::error!("获取数据库连接失败: {:?}", e);
        anyhow::anyhow!("获取数据库连接失败")
    })?.as_ref().context("获取数据库连接as_ref失败")?.get()?)
}

pub fn close_pool() {
    info!("关闭连接池...");
    if let Some(pool_arc) = POOL.get() {
        if let Ok(mut pool_option_lock) = pool_arc.lock() {
            let pool_option = pool_option_lock.take();
            if pool_option.is_some() {
                info!("数据库连接池已关闭。");
            }
        }
    }
}
