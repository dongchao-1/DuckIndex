use anyhow::Result;
use rusqlite::params;
use serde::{Deserialize, Serialize};
use indexmap::IndexMap;

use crate::sqlite::get_conn;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Config {}

impl Config {
    fn get_key(key: &str) -> Result<String> {
        let conn = get_conn()?;
        let value: String = conn.query_one(
            "SELECT value FROM config WHERE key = ?1",
            params![key],
            |row| row.get(0),
        )?;
        Ok(value)
    }

    fn set_key(key: &str, value: &str) -> Result<()> {
        let conn = get_conn()?;
        conn.execute(
            "UPDATE config SET value = ?2 WHERE key = ?1",
            params![key, value],
        )?;
        Ok(())
    }

    pub fn get_index_dir_paths() -> Result<Vec<String>> {
        Self::get_key("index_dir_paths").and_then(|s| {
            let v: Vec<String> = serde_json::from_str(&s)?;
            Ok(v)
        })
    }
    pub fn set_index_dir_paths(index_dir_paths: Vec<String>) -> Result<()> {
        Self::set_key(
            "index_dir_paths",
            &serde_json::to_string(&index_dir_paths)?,
        )
    }

    pub fn set_extension_whitelist(extension_whitelist: IndexMap<String, IndexMap<String, bool>>) -> Result<()> {
        Self::set_key(
            "extension_whitelist",
            &serde_json::to_string(&extension_whitelist)?,
        )
    }
    pub fn get_extension_whitelist() -> Result<IndexMap<String, IndexMap<String, bool>>> {
        Self::get_key("extension_whitelist").and_then(|s| {
            let v: IndexMap<String, IndexMap<String, bool>> = serde_json::from_str(&s)?;
            Ok(v)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test::test_mod::TestEnv;

    #[test]
    fn test_get_set_key() {
        let _env = TestEnv::new();
        let test_value = Config::get_key("index_dir_paths").unwrap();
        assert_eq!(test_value, "[]");

        Config::set_key("index_dir_paths", "test_value").unwrap();

        let test_value = Config::get_key("index_dir_paths").unwrap();
        assert_eq!(test_value, "test_value");
    }

    #[test]
    fn test_get_set_index_dir_paths() {
        let _env = TestEnv::new();
        let index_dir_paths = Config::get_index_dir_paths().unwrap();
        assert_eq!(index_dir_paths, Vec::<String>::new());

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

    #[test]
    fn test_get_set_extension_whitelist() {
        let _env = TestEnv::new();
        let extension_whitelist = Config::get_extension_whitelist().unwrap();
        assert_eq!(extension_whitelist, IndexMap::<String, IndexMap<String, bool>>::new());

        let mut doc_extensions = IndexMap::new();
        doc_extensions.insert("docx".into(), true);
        doc_extensions.insert("doc".into(), false);

        let mut new_whitelist = IndexMap::new();
        new_whitelist.insert("文档".into(), doc_extensions);

        Config::set_extension_whitelist(new_whitelist.clone()).unwrap();
        
        let result = Config::get_extension_whitelist().unwrap();
        assert_eq!(new_whitelist, result);
    }
}
