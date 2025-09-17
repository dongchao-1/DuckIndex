use anyhow::Result;
use log::info;
use rusqlite::params;
use serde::{Deserialize, Serialize};
use strum::Display;
use strum::EnumString;

use crate::sqlite::get_conn;

pub struct Config {}

#[derive(Debug, PartialEq, EnumString, Display)]
enum ConfigKey {
    #[strum(to_string = "IndexDirPaths")]
    IndexDirPaths,
    #[strum(to_string = "ExtensionWhitelist")]
    ExtensionWhitelist,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq)]
pub struct ExtensionConfigTree {
    pub label: String,
    pub is_extension: bool,
    pub children: Option<Vec<ExtensionConfigTree>>,
    pub enabled: Option<bool>,
}

impl Config {
    fn get_key<T>(key: &ConfigKey) -> Result<T>
    where
        T: serde::de::DeserializeOwned,
    {
        let conn = get_conn()?;
        let value: String = conn.query_one(
            "SELECT value FROM config WHERE key = ?1",
            params![key.to_string()],
            |row| row.get(0),
        )?;
        let v: T = serde_json::from_str(&value)?;
        Ok(v)
    }

    fn set_key<T>(key: &ConfigKey, value: &T) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        let v = serde_json::to_string(value)?;
        let conn = get_conn()?;
        conn.execute(
            "UPDATE config SET value = ?2 WHERE key = ?1",
            params![key.to_string(), v],
        )?;
        Ok(())
    }

    pub fn get_index_dir_paths() -> Result<Vec<String>> {
        Self::get_key(&ConfigKey::IndexDirPaths)
    }
    pub fn set_index_dir_paths(index_dir_paths: Vec<String>) -> Result<()> {
        Self::set_key(&ConfigKey::IndexDirPaths, &index_dir_paths)
    }

    pub fn get_extension_whitelist() -> Result<Vec<ExtensionConfigTree>> {
        Self::get_key(&ConfigKey::ExtensionWhitelist)
    }

    pub fn set_extension_enabled(extension: &str, enabled: bool) -> Result<()> {
        let mut extension_whitelist = Self::get_extension_whitelist()?;

        fn find_and_set_enabled(
            nodes: &mut [ExtensionConfigTree],
            target_label: &str,
            enabled: bool,
        ) -> bool {
            for node in nodes.iter_mut() {
                if node.label == target_label && node.is_extension {
                    node.enabled = Some(enabled);
                    info!("设置文件类型: {target_label} 状态为: {enabled}");
                    return true;
                }
                if let Some(ref mut children) = node.children {
                    if find_and_set_enabled(children, target_label, enabled) {
                        return true;
                    }
                }
            }
            false
        }

        if find_and_set_enabled(&mut extension_whitelist, extension, enabled) {
            Self::set_extension_whitelist(&extension_whitelist)?;
            Ok(())
        } else {
            Err(anyhow::anyhow!(
                "Extension '{}' not found in whitelist",
                extension
            ))
        }
    }

    fn set_extension_whitelist(extension_whitelist: &Vec<ExtensionConfigTree>) -> Result<()> {
        Self::set_key(&ConfigKey::ExtensionWhitelist, &extension_whitelist)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test::test_mod::TestEnv;

    #[test]
    fn test_get_set_key() {
        let _env = TestEnv::new();
        let test_value: Vec<String> = Config::get_key(&ConfigKey::IndexDirPaths).unwrap();
        assert_eq!(test_value, Vec::<String>::new());

        Config::set_key(&ConfigKey::IndexDirPaths, &vec!["test_value".to_string()]).unwrap();

        let test_value: Vec<String> = Config::get_key(&ConfigKey::IndexDirPaths).unwrap();
        assert_eq!(test_value, vec!["test_value".to_string()]);
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
        let _env = TestEnv::new_with_cleanup(false);
        let extension_whitelist = vec![
            ExtensionConfigTree {
                label: "文档".into(),
                is_extension: false,
                children: Some(vec![
                    ExtensionConfigTree {
                        label: "docx".into(),
                        is_extension: true,
                        children: None,
                        enabled: Some(true),
                    },
                    ExtensionConfigTree {
                        label: "doc".into(),
                        is_extension: true,
                        children: None,
                        enabled: Some(false),
                    },
                ]),
                enabled: None,
            },
            ExtensionConfigTree {
                label: "数据".into(),
                is_extension: false,
                children: Some(vec![ExtensionConfigTree {
                    label: "xlsx".into(),
                    is_extension: true,
                    children: None,
                    enabled: Some(false),
                }]),
                enabled: None,
            },
        ];

        Config::set_extension_whitelist(&extension_whitelist).unwrap();

        let result = Config::get_extension_whitelist().unwrap();
        assert_eq!(extension_whitelist, result);
    }

    #[test]
    fn test_set_extension_enabled() {
        let _env = TestEnv::new_with_cleanup(false);
        let extension_whitelist = vec![ExtensionConfigTree {
            label: "文档".into(),
            is_extension: false,
            children: Some(vec![
                ExtensionConfigTree {
                    label: "docx".into(),
                    is_extension: true,
                    children: None,
                    enabled: Some(false), // 初始为 false
                },
                ExtensionConfigTree {
                    label: "doc".into(),
                    is_extension: true,
                    children: None,
                    enabled: Some(true), // 初始为 true
                },
            ]),
            enabled: None,
        }];

        Config::set_extension_whitelist(&extension_whitelist).unwrap();

        // 启用 docx
        Config::set_extension_enabled("docx", true).unwrap();
        let result = Config::get_extension_whitelist().unwrap();
        let docx_node = result[0]
            .children
            .as_ref()
            .unwrap()
            .iter()
            .find(|node| node.label == "docx")
            .unwrap();
        assert_eq!(docx_node.enabled, Some(true));

        // 禁用 doc
        Config::set_extension_enabled("doc", false).unwrap();
        let result = Config::get_extension_whitelist().unwrap();
        let doc_node = result[0]
            .children
            .as_ref()
            .unwrap()
            .iter()
            .find(|node| node.label == "doc")
            .unwrap();
        assert_eq!(doc_node.enabled, Some(false));

        // 测试不存在的扩展名
        let error = Config::set_extension_enabled("nonexistent", true).unwrap_err();
        assert!(error.to_string().contains("not found"));
    }
}
