use anyhow::{anyhow, Context, Result};
use chrono::{DateTime, Local};
use log::{debug, info};
use rusqlite::params;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, MAIN_SEPARATOR};

use crate::reader::Item;
use crate::sqlite::get_conn;
use crate::utils::{filename_to_str, parent_to_str, path_to_str};

#[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize)]
pub struct SearchResultDirectory {
    pub name: String,
    pub path: String,
    pub modified_time: String,
}

#[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize)]
pub struct SearchResultFile {
    pub name: String,
    pub path: String,
    pub modified_time: String,
}

#[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize)]
pub struct SearchResultItem {
    pub content: String,
    pub file: String,
    pub path: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct IndexStatusStat {
    pub directories: usize,
    pub files: usize,
    pub items: usize,
}

pub struct Indexer {}

impl Indexer {
    pub fn new() -> Result<Self> {
        Ok(Indexer {})
    }

    fn check_is_absolute(&self, path: &Path) -> Result<()> {
        if !path.is_absolute() {
            return Err(anyhow!("Path {} is not an absolute path", path.display()));
        }
        Ok(())
    }

    pub fn get_modified_time(&self, path: &Path) -> Result<String> {
        let modified_datetime: DateTime<Local> = DateTime::from(fs::metadata(path)?.modified()?);
        Ok(modified_datetime.to_rfc3339())
    }

    pub fn write_directory(&self, directory: &Path) -> Result<i64> {
        self.check_is_absolute(directory)?;
        let dir_name = filename_to_str(directory)?;
        let dir_path = path_to_str(directory)?;
        let modified_time = self.get_modified_time(directory)?;

        let directory_id = get_conn()?.query_row(
            "INSERT INTO directories (name, path, modified_time) VALUES (?1, ?2, ?3) ON CONFLICT(path) DO UPDATE SET modified_time = ?3 RETURNING id",
            params![&dir_name, &dir_path, &modified_time],
            |row| row.get(0)
        )?;
        Ok(directory_id)
    }

    pub fn get_directory(&self, directory: &Path) -> Result<SearchResultDirectory> {
        self.check_is_absolute(directory)?;
        let dir_path = path_to_str(directory)?;
        let conn = get_conn()?;
        let mut stmt =
            conn.prepare("SELECT name, path, modified_time FROM directories WHERE path = ?1")?;
        let row = stmt.query_row(params![dir_path], |row| {
            Ok(SearchResultDirectory {
                name: row.get(0)?,
                path: row.get(1)?,
                modified_time: row.get(2)?,
            })
        })?;
        Ok(row)
    }

    pub fn get_file(&self, file: &Path) -> Result<SearchResultFile> {
        self.check_is_absolute(file)?;
        let file_path = parent_to_str(file)?;
        let file_name = filename_to_str(file)?;
        let conn = get_conn()?;
        let mut stmt = conn.prepare(
            r"SELECT files.name, directories.path, files.modified_time 
            FROM files
            join directories
            on files.directory_id = directories.id
            WHERE directories.path = ?1 and files.name = ?2",
        )?;
        let row = stmt.query_row(params![file_path, file_name], |row| {
            Ok(SearchResultFile {
                name: row.get(0)?,
                path: row.get(1)?,
                modified_time: row.get(2)?,
            })
        })?;
        Ok(row)
    }

    pub fn write_file_items(&self, file: &Path, items: Vec<Item>) -> Result<i64> {
        self.check_is_absolute(file)?;
        let parent_dir = file.parent().with_context(|| format!("Failed to get parent directory from file: {}", file.display()))?;
        let directory_id = self.write_directory(parent_dir)?;

        let file_name = filename_to_str(file)?;
        let modified_time = self.get_modified_time(file)?;

        let mut conn = get_conn()?;
        let tx = conn.transaction()?;
        let file_id: i64 = tx.query_row(
            "INSERT INTO files (directory_id, name, modified_time) VALUES (?1, ?2, ?3) ON CONFLICT(directory_id, name) DO UPDATE SET modified_time = ?3 RETURNING id",
            params![&directory_id, file_name, &modified_time],
            |row| row.get(0),
        )?;
        // println!("write_file_items File ID: {}", file_id);

        for chunk in items.chunks(1000) {
            let mut query =
                String::from("INSERT INTO items (file_id, content) VALUES ");

            // 构建 VALUES 部分 (?, ?, ?, ?), (?, ?, ?, ?), ...
            let values: Vec<String> = (0..chunk.len())
                .map(|i| {
                    let base = i * 2 + 1; // 每个 item 有 2 个参数
                    format!("(?{}, ?{})", base, base + 1)
                })
                .collect();
            query.push_str(&values.join(", "));

            // 准备所有参数
            let mut params = Vec::new();
            for item in chunk.iter() {
                params.push(&file_id as &dyn rusqlite::ToSql);
                params.push(&item.content as &dyn rusqlite::ToSql);
            }

            // 执行批量插入
            tx.execute(&query, params.as_slice())?;
        }
        tx.commit()?;
        Ok(file_id)
    }

    pub fn get_sub_directories_and_files(
        &self,
        directory: &Path,
    ) -> Result<(Vec<SearchResultDirectory>, Vec<SearchResultFile>)> {
        self.check_is_absolute(directory)?;

        let mut dirs = Vec::new();
        let mut files = Vec::new();

        let dir_path = path_to_str(directory)?;
        let conn = get_conn()?;
        let mut stmt = conn.prepare(
            "SELECT name, path, modified_time FROM directories WHERE path LIKE ?1 AND path NOT LIKE ?2",
        )?;
        let rows = stmt.query_map(
            params![format!("{}{}%", dir_path, MAIN_SEPARATOR), format!("{}{}%{}%", dir_path, MAIN_SEPARATOR, MAIN_SEPARATOR)],
            |row| {
            Ok(SearchResultDirectory {
                name: row.get(0)?,
                path: row.get(1)?,
                modified_time: row.get(2)?,
            })
        })?;

        for row in rows {
            dirs.push(row?);
        }

        let mut stmt = conn.prepare(
            r"SELECT files.name, directories.path, files.modified_time 
            FROM files
            JOIN directories
            ON files.directory_id = directories.id
            WHERE directories.path = ?1",
        )?;
        let rows = stmt.query_map(
            params![dir_path],
            |row| {
                Ok(SearchResultFile {
                    name: row.get(0)?,
                    path: row.get(1)?,
                    modified_time: row.get(2)?,
                })
            },
        )?;

        for row in rows {
            files.push(row?);
        }

        Ok((dirs, files))
    }

    pub fn search_directory(
        &self,
        content: &str,
        offset: usize,
        limit: usize,
    ) -> Result<Vec<SearchResultDirectory>> {
        let mut result = Vec::new();
        let conn = get_conn()?;

        let sql = format!(
            "SELECT name, path, modified_time FROM directories WHERE name LIKE '%{content}%' ORDER BY id LIMIT {limit} OFFSET {offset}"
        );
        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt.query_map([], |row| {
            Ok(SearchResultDirectory {
                name: row.get(0)?,
                path: row.get(1)?,
                modified_time: row.get(2)?,
            })
        })?;

        for row in rows {
            result.push(row.context("Failed to map row to SearchResultDirectory")?);
        }
        Ok(result)
    }

    pub fn search_file(
        &self,
        content: &str,
        offset: usize,
        limit: usize,
    ) -> Result<Vec<SearchResultFile>> {
        let mut result = Vec::new();
        let conn = get_conn()?;

        let sql = format!(
            r"SELECT files.name, directories.path, files.modified_time
            FROM files
            left outer join directories
            on files.directory_id = directories.id
            WHERE files.name LIKE '%{content}%' ORDER BY files.id LIMIT {limit} OFFSET {offset}"
        );
        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt.query_map([], |row| {
            Ok(SearchResultFile {
                name: row.get(0)?,
                path: row.get(1)?,
                modified_time: row.get(2)?,
            })
        })?;

        for row in rows {
            result.push(row.context("Failed to map row to SearchResultFile")?);
        }
        Ok(result)
    }

    pub fn search_item(
        &self,
        content: &str,
        offset: usize,
        limit: usize,
    ) -> Result<Vec<SearchResultItem>> {
        let mut result = Vec::new();
        let conn = get_conn()?;

        let sql = format!(
            r"SELECT items.content, files.name, directories.path
            FROM items
            LEFT OUTER JOIN files ON items.file_id = files.id
            LEFT OUTER JOIN directories ON files.directory_id = directories.id
            WHERE items.content LIKE '%{content}%' ORDER BY items.id LIMIT {limit} OFFSET {offset}"
        );
        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt.query_map([], |row| {
            Ok(SearchResultItem {
                content: row.get(0)?,
                file: row.get(1)?,
                path: row.get(2)?,
            })
        })?;

        for row in rows {
            result.push(row.context("Failed to map row to SearchResultItem")?);
        }
        Ok(result)
    }

    pub fn delete_file(&self, file: &Path) -> Result<()> {
        self.check_is_absolute(file)?;
        let file_name = filename_to_str(file)?;
        let directory_path = parent_to_str(file)?;
        let mut conn = get_conn()?;
        let tx = conn.transaction()?;

        tx.execute(
            r"DELETE FROM items WHERE file_id in 
            (SELECT id FROM files WHERE name = ?1 and directory_id in (SELECT id FROM directories WHERE path = ?2))",
            params![&file_name, &directory_path],
        )?;

        tx.execute(r"DELETE FROM files WHERE name = ?1 
            and directory_id in (SELECT id FROM directories WHERE path = ?2)", 
            params![&file_name, &directory_path])?;
        tx.commit()?;

        Ok(())
    }

    pub fn delete_directory(&self, directory: &Path) -> Result<()> {
        self.check_is_absolute(directory)?;

        debug!("查找子目录和文件: {}", directory.display());
        let (sub_dirs, files) = self.get_sub_directories_and_files(directory)?;

        for file in files {
            info!("删除文件: {}", file.name);
            self.delete_file(&Path::new(&file.path).join(&file.name))?;
        }

        for sub_dir in sub_dirs {
            info!("删除子目录: {}", sub_dir.path);
            self.delete_directory(Path::new(&sub_dir.path))?;
        }

        info!("删除目录记录: {}", directory.display());
        let dir_path = path_to_str(directory)?;
        let conn = get_conn()?;
        conn.execute("DELETE FROM directories WHERE path = ?1", params![dir_path])?;

        Ok(())
    }

    pub fn get_index_status(&self) -> Result<IndexStatusStat> {
        let conn = get_conn()?;
        let total_directories: i64 = conn.query_one("SELECT COUNT(*) FROM directories", [], |row| row.get(0))?;
        let total_files: i64 = conn.query_one("SELECT COUNT(*) FROM files", [], |row| row.get(0))?;
        let indexed_files: i64 = conn.query_one("SELECT COUNT(*) FROM items", [], |row| row.get(0))?;
        Ok(IndexStatusStat {
            directories: total_directories as usize,
            files: total_files as usize,
            items: indexed_files as usize,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test::test_mod::TestEnv;

    const TEST_DATA_DIR: &str = "../test_data/indexer";

    #[test]
    fn test_get_index() {
        let _env = TestEnv::new();
        let _ = Indexer::new().unwrap();
    }

    #[test]
    fn test_write_directory() {
        let _env = TestEnv::new();
        let indexer = Indexer::new().unwrap();
        let path = Path::new(TEST_DATA_DIR).canonicalize().unwrap();
        indexer.write_directory(&path).unwrap();
    }

    #[test]
    fn test_get_directory() {
        let _env = TestEnv::new();
        let indexer = Indexer::new().unwrap();
        let path = Path::new(TEST_DATA_DIR).canonicalize().unwrap();
        indexer.write_directory(&path).unwrap();

        let dir = indexer.get_directory(&path).unwrap();
        assert_eq!(dir.name, "indexer");
        assert_eq!(dir.path, path.canonicalize().unwrap().to_str().unwrap());
    }

    #[test]
    fn test_write_file_items() {
        let _env = TestEnv::new();
        let indexer = Indexer::new().unwrap();

        let file = Path::new(TEST_DATA_DIR).join("1.txt").canonicalize().unwrap();
        indexer.write_directory(file.parent().unwrap()).unwrap();

        let items = vec![
            Item {
                content: "Hello, world!".into(),
            },
            Item {
                content: "This is a test.".into(),
            },
        ];
        indexer.write_file_items(&file, items).unwrap();
    }

    #[test]
    fn test_get_file() {
        let _env = TestEnv::new();
        let indexer = Indexer::new().unwrap();

        let file = Path::new(TEST_DATA_DIR).join("1.txt").canonicalize().unwrap();
        indexer.write_directory(file.parent().unwrap()).unwrap();

        let items = vec![
            Item {
                content: "Hello, world!".into(),
            },
            Item {
                content: "This is a test.".into(),
            },
        ];
        indexer.write_file_items(&file, items).unwrap();

        let file_result = indexer.get_file(&file).unwrap();
        assert_eq!(file_result.name, "1.txt");
        assert_eq!(file_result.path, file.parent().unwrap().to_str().unwrap());
    }

    #[test]
    fn test_get_sub_directories_and_files() {
        let _env = TestEnv::new();
        let indexer = Indexer::new().unwrap();

        let file = Path::new(TEST_DATA_DIR).join("1.txt").canonicalize().unwrap();
        indexer.write_directory(file.parent().unwrap()).unwrap();

        let items = vec![
            Item {
                content: "Hello, world!".into(),
            },
            Item {
                content: "This is a test.".into(),
            },
        ];
        indexer.write_file_items(&file, items).unwrap();

        let sub_dir_path = Path::new(TEST_DATA_DIR).join("office").canonicalize().unwrap();
        indexer.write_directory(&sub_dir_path).unwrap();

        let (dir_result, file_result) = indexer
            .get_sub_directories_and_files(file.parent().unwrap())
            .unwrap();
        assert_eq!(dir_result.len(), 1);
        assert_eq!(file_result.len(), 1);
    }

    #[test]
    fn test_search_directory() {
        let _env = TestEnv::new();
        let indexer = Indexer::new().unwrap();
        let dir = Path::new(TEST_DATA_DIR).canonicalize().unwrap();
        indexer.write_directory(&dir).unwrap();

        let result = indexer.search_directory("indexer", 0, 10).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].name, "indexer");

        let result = indexer.search_directory("indexer", 1, 10).unwrap();
        assert_eq!(result.len(), 0);
    }

    #[test]
    fn test_search_file() {
        let _env = TestEnv::new();
        let indexer = Indexer::new().unwrap();
        let items = vec![
            Item {
                content: "Hello, world!".into(),
            },
            Item {
                content: "This is a test.".into(),
            },
        ];
        let file = Path::new(TEST_DATA_DIR).join("1.txt").canonicalize().unwrap();
        indexer.write_directory(file.parent().unwrap()).unwrap();
        indexer.write_file_items(&file, items).unwrap();

        let result = indexer.search_file("1.t", 0, 10).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].name, "1.txt");
        assert_eq!(result[0].path, file.parent().unwrap().to_str().unwrap());

        let result = indexer.search_file("1.t", 1, 10).unwrap();
        assert_eq!(result.len(), 0);
    }

    #[test]
    fn test_search_item() {
        let _env = TestEnv::new();
        let indexer = Indexer::new().unwrap();
        let items = vec![
            Item {
                content: "Hello, world!".into(),
            },
            Item {
                content: "This is a test.".into(),
            },
        ];
        let file = Path::new(TEST_DATA_DIR).join("1.txt").canonicalize().unwrap();
        indexer.write_directory(file.parent().unwrap()).unwrap();
        indexer.write_file_items(&file, items).unwrap();

        let result = indexer.search_item("world", 0, 10).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].content, "Hello, world!");
        assert_eq!(result[0].file, "1.txt");
        assert_eq!(result[0].path, file.parent().unwrap().to_str().unwrap());
    }

    #[test]
    fn test_delete_file() {
        let _env = TestEnv::new();
        let indexer = Indexer::new().unwrap();
        let items = vec![
            Item {
                content: "Hello, world!".into(),
            },
            Item {
                content: "This is a test.".into(),
            },
        ];
        let file = Path::new(TEST_DATA_DIR).join("1.txt").canonicalize().unwrap();
        indexer.write_directory(file.parent().unwrap()).unwrap();
        indexer.write_file_items(&file, items).unwrap();

        indexer.delete_file(&file).unwrap();

        let (dir_result, file_result) = indexer
            .get_sub_directories_and_files(file.parent().unwrap())
            .unwrap();
        assert_eq!(dir_result.len(), 0);
        assert_eq!(file_result.len(), 0);
    }

    
    #[test]
    fn test_delete_file_not_exists() {
        let _env = TestEnv::new();
        let indexer = Indexer::new().unwrap();
        let items = vec![
            Item {
                content: "Hello, world!".into(),
            },
            Item {
                content: "This is a test.".into(),
            },
        ];
        let file = Path::new(TEST_DATA_DIR).join("1.txt").canonicalize().unwrap();
        indexer.write_directory(file.parent().unwrap()).unwrap();
        indexer.write_file_items(&file, items).unwrap();

        indexer.delete_file(&file.parent().unwrap().join("non_existent.txt")).unwrap();
    }

    #[test]
    fn test_delete_directory() {
        let _env = TestEnv::new();
        let indexer = Indexer::new().unwrap();
        let items = vec![
            Item {
                content: "Hello, world!".into(),
            },
            Item {
                content: "This is a test.".into(),
            },
        ];
        let file = Path::new(TEST_DATA_DIR).join("1.txt").canonicalize().unwrap();
        indexer.write_directory(file.parent().unwrap()).unwrap();
        indexer.write_file_items(&file, items).unwrap();
        indexer
            .write_directory(&Path::new(TEST_DATA_DIR).join("office").canonicalize().unwrap())
            .unwrap();

        indexer.delete_directory(file.parent().unwrap()).unwrap();

        let (dir_result, file_result) = indexer
            .get_sub_directories_and_files(file.parent().unwrap())
            .unwrap();
        assert_eq!(dir_result.len(), 0);
        assert_eq!(file_result.len(), 0);
    }


    #[test]
    fn test_delete_directory_not_exists() {
        let _env = TestEnv::new();
        let indexer = Indexer::new().unwrap();
        let items = vec![
            Item {
                content: "Hello, world!".into(),
            },
            Item {
                content: "This is a test.".into(),
            },
        ];
        let file = Path::new(TEST_DATA_DIR).join("1.txt").canonicalize().unwrap();
        indexer.write_directory(file.parent().unwrap()).unwrap();
        indexer.write_file_items(&file, items).unwrap();
        indexer
            .write_directory(&Path::new(TEST_DATA_DIR).join("office").canonicalize().unwrap())
            .unwrap();

        indexer.delete_directory(&file.parent().unwrap().join("not_exists_path")).unwrap();
    }

    #[test]
    fn test_get_index_status() {
        let _env = TestEnv::new();
        let indexer = Indexer::new().unwrap();
        let items = vec![
            Item {
                content: "Hello, world!".into(),
            },
            Item {
                content: "This is a test.".into(),
            },
        ];
        let file = Path::new(TEST_DATA_DIR).join("1.txt").canonicalize().unwrap();
        indexer.write_directory(file.parent().unwrap()).unwrap();
        indexer.write_file_items(&file, items).unwrap();

        let result = indexer.get_index_status().unwrap();
        assert_eq!(result.directories, 1);
        assert_eq!(result.files, 1);
        assert_eq!(result.items, 2);
    }

}
