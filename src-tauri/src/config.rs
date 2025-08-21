use serde::{Deserialize, Serialize};
use anyhow::{anyhow, Result};

use crate::sqlite::get_pool;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Config {
}

impl Config {
    
    pub fn check_or_init() -> Result<()> {
        if let Err(_) = Self::check_config_init() {
            Self::reset_config()?;
        }
        Ok(())
    }
    
    fn check_config_init() -> Result<()> {
        let conn = get_pool()?;
        let row = conn.query_one("select version from config_version", [], |row|
            row.get::<_, String>(0)
        ).map_err(|e| anyhow!("Config not initialized: {}", e))?;

        if row != "0.1" {
            return Err(anyhow!("Config version mismatch: expected 0.1, found {}", row));
        }
        Ok(())
    }

    fn reset_config() -> Result<()> {
        let conn = get_pool()?;
        conn.execute_batch(
            r"
            DROP TABLE IF EXISTS config;
            CREATE TABLE config (
                id INTEGER PRIMARY KEY,
                key TEXT NOT NULL,
                value TEXT NOT NULL,
                unique (key)
            );
            INSERT INTO config (key, value) VALUES ('index_dir_paths', '[]');

            DROP TABLE IF EXISTS config_version;
            CREATE TABLE config_version (
                version TEXT
            );
            INSERT INTO config_version (version) VALUES ('0.1');
        ")?;
        Ok(())
    }

    pub fn get_index_dir_paths() -> Result<Vec<String>> {
        let conn = get_pool()?;
        let index_dir_paths: String = conn.query_one("SELECT value FROM config WHERE key = 'index_dir_paths'", [], |row| 
            row.get(0)
        )?;
        let index_dir_paths: Vec<String> = serde_json::from_str(&index_dir_paths)?;
        Ok(index_dir_paths)
    }

    pub fn set_index_dir_paths(index_dir_paths: Vec<String>) -> Result<()> {
        let conn = get_pool()?;
        let index_dir_paths = serde_json::to_string(&index_dir_paths)?;
        conn.execute("update config set value = $1 where key = 'index_dir_paths'", [index_dir_paths])?;
        Ok(())
    }

}



#[cfg(test)]
mod tests {
    use super::*;
    use crate::test::test::TestEnv;

    #[test]
    fn test_get_index_dir_paths() {
        let _env = TestEnv::new();
        let index_dir_paths = Config::get_index_dir_paths().unwrap();
        assert_eq!(index_dir_paths, Vec::<String>::new());
    }

    #[test]
    fn test_set_index_dir_paths() {
        let _env = TestEnv::new();
        let result = Config::set_index_dir_paths(vec!["/path/to/index".into(), "/path/to/another/index".into()]);
        assert!(result.is_ok());

        let index_dir_paths = Config::get_index_dir_paths().unwrap();
        assert_eq!(index_dir_paths, vec![String::from("/path/to/index"), String::from("/path/to/another/index")]);
    }
}
