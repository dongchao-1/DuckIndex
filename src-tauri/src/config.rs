use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::sqlite::get_conn;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Config {}

impl Config {
    pub fn get_index_dir_paths() -> Result<Vec<String>> {
        let conn = get_conn()?;
        let index_dir_paths: String = conn.query_one(
            "SELECT value FROM config WHERE key = 'index_dir_paths'",
            [],
            |row| row.get(0),
        )?;
        let index_dir_paths: Vec<String> = serde_json::from_str(&index_dir_paths)?;
        Ok(index_dir_paths)
    }

    pub fn set_index_dir_paths(index_dir_paths: Vec<String>) -> Result<()> {
        let conn = get_conn()?;
        let index_dir_paths_str = serde_json::to_string(&index_dir_paths)?;
        conn.execute(
            "update config set value = $1 where key = 'index_dir_paths'",
            [index_dir_paths_str],
        )?;
        
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
        let result = Config::set_index_dir_paths(vec![
            "../test_data/indexer".into(),
            "../test_data/reader".into(),
        ]);
        assert!(result.is_ok());

        let index_dir_paths = Config::get_index_dir_paths().unwrap();
        assert_eq!(
            index_dir_paths,
            vec![
                String::from("../test_data/indexer"),
                String::from("../test_data/reader")
            ]
        );
    }
}
