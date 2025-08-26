use anyhow::{anyhow, Result};
use chrono::{DateTime, Local};
use rusqlite::params;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

use crate::reader::Item;
use crate::sqlite::get_pool;

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
    pub fn check_or_init() -> Result<()> {
        if let Err(_) = Self::check_indexer_init() {
            Self::reset_indexer()?;
        }
        Ok(())
    }

    fn check_indexer_init() -> Result<()> {
        let conn = get_pool()?;
        let row = conn
            .query_one("select version from indexer_version", [], |row| {
                row.get::<_, String>(0)
            })
            .map_err(|e| anyhow!("Indexer not initialized: {}", e))?;

        if row != "0.1" {
            return Err(anyhow!(
                "Indexer version mismatch: expected 0.1, found {}",
                row
            ));
        }
        Ok(())
    }

    fn reset_indexer() -> Result<()> {
        let conn = get_pool()?;
        conn.execute_batch(
            r"
            DROP TABLE IF EXISTS directories;
            CREATE TABLE directories (
                id INTEGER PRIMARY KEY,
                name TEXT NOT NULL,
                path TEXT NOT NULL,
                modified_time TEXT NOT NULL,
                UNIQUE (path)
            );
            DROP TABLE IF EXISTS files;
            CREATE TABLE files (
                id INTEGER PRIMARY KEY,
                directory_id INTEGER NOT NULL,
                name TEXT NOT NULL,
                modified_time TEXT NOT NULL,
                UNIQUE (directory_id, name)
            );
            DROP TABLE IF EXISTS items;
            CREATE TABLE items (
                id INTEGER PRIMARY KEY,
                file_id INTEGER NOT NULL,
                content TEXT NOT NULL
            );
            DROP TABLE IF EXISTS indexer_version;
            CREATE TABLE indexer_version (
                version TEXT
            );
            INSERT INTO indexer_version (version) VALUES ('0.1');
        ",
        )?;
        Ok(())
    }

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
        let dir_name = directory.file_name().unwrap().to_str().unwrap();
        let dir_path = directory.to_str().unwrap();
        let modified_time = self.get_modified_time(directory)?;

        let directory_id = get_pool()?.query_row(
            "INSERT INTO directories (name, path, modified_time) VALUES (?1, ?2, ?3) ON CONFLICT(path) DO UPDATE SET path = path RETURNING id",
            params![&dir_name, &dir_path, &modified_time],
            |row| row.get(0)
        )?;
        Ok(directory_id)
    }

    pub fn get_directory(&self, directory: &Path) -> Result<SearchResultDirectory> {
        self.check_is_absolute(directory)?;
        let dir_path = directory.to_str().unwrap();
        let conn = get_pool()?;
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
        let file_path = file.parent().unwrap().to_str().unwrap();
        let file_name = file.file_name().unwrap().to_str().unwrap();
        let conn = get_pool()?;
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

    pub fn write_file_items(&self, file: &Path, items: Vec<Item>) -> Result<()> {
        self.check_is_absolute(file)?;
        let directory_id = self.write_directory(file.parent().unwrap())?;

        let file_name = file.file_name().unwrap().to_str().unwrap();
        let modified_time = self.get_modified_time(&file)?;

        let mut conn = get_pool()?;
        let tx = conn.transaction()?;
        let file_id: i64 = tx.query_row(
            "INSERT INTO files (directory_id, name, modified_time) VALUES (?1, ?2, ?3) ON CONFLICT(directory_id, name) DO UPDATE SET directory_id = directory_id RETURNING id",
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
        Ok(())
    }

    pub fn get_sub_directories_and_files(
        &self,
        directory: &Path,
    ) -> Result<(Vec<SearchResultDirectory>, Vec<SearchResultFile>)> {
        self.check_is_absolute(directory)?;

        let mut dirs = Vec::new();
        let mut files = Vec::new();

        let dir_path = directory.to_str().unwrap();
        let conn = get_pool()?;
        let mut stmt = conn.prepare(
            "SELECT name, path, modified_time FROM directories WHERE path != ?1 AND path LIKE ?2",
        )?;
        let rows = stmt.query_map(params![dir_path, format!("{}%", dir_path)], |row| {
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
        let rows = stmt.query_map(params![dir_path], |row| {
            Ok(SearchResultFile {
                name: row.get(0)?,
                path: row.get(1)?,
                modified_time: row.get(2)?,
            })
        })?;

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
        let conn = get_pool()?;

        let sql = format!(
            "SELECT name, path, modified_time FROM directories WHERE name LIKE '%{}%' ORDER BY id LIMIT {} OFFSET {}",
            content, limit, offset
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
            result.push(row.unwrap());
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
        let conn = get_pool()?;

        let sql = format!(
            r"SELECT files.name, directories.path, files.modified_time
            FROM files
            left outer join directories
            on files.directory_id = directories.id
            WHERE files.name LIKE '%{}%' ORDER BY files.id LIMIT {} OFFSET {}",
            content, limit, offset
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
            result.push(row.unwrap());
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
        let conn = get_pool()?;

        let sql = format!(
            r"SELECT items.content, files.name, directories.path
            FROM items
            LEFT OUTER JOIN files ON items.file_id = files.id
            LEFT OUTER JOIN directories ON files.directory_id = directories.id
            WHERE items.content LIKE '%{}%' ORDER BY items.id LIMIT {} OFFSET {}",
            content, limit, offset
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
            result.push(row.unwrap());
        }
        Ok(result)
    }

    pub fn delete_file(&self, file: &Path) -> Result<()> {
        self.check_is_absolute(file)?;
        let file_name = file.file_name().unwrap().to_str().unwrap();
        let directory_path = file.parent().unwrap().to_str().unwrap();
        let mut conn = get_pool()?;
        let tx = conn.transaction()?;
        let file_id: i64 = tx.query_row(
            "SELECT id FROM files WHERE name = ?1 and directory_id in (SELECT id FROM directories WHERE path = ?2)",
            params![file_name, &directory_path],
            |row| row.get(0),
        )?;

        tx.execute(
            "DELETE FROM items WHERE file_id = ?1",
            &[&file_id.to_string()],
        )?;

        tx.execute("DELETE FROM files WHERE id = ?1", &[&file_id.to_string()])?;
        tx.commit()?;

        Ok(())
    }

    pub fn delete_directory(&self, directory: &Path) -> Result<()> {
        self.check_is_absolute(&directory)?;

        let (sub_dirs, files) = self.get_sub_directories_and_files(&directory)?;

        for file in files {
            self.delete_file(&Path::new(&file.path).join(&file.name))?;
        }

        for sub_dir in sub_dirs {
            self.delete_directory(Path::new(&sub_dir.path))?;
        }

        let dir_path = directory.to_str().unwrap();
        let conn = get_pool()?;
        conn.execute("DELETE FROM directories WHERE path = ?1", params![dir_path])?;

        Ok(())
    }

    pub fn get_index_status(&self) -> Result<IndexStatusStat> {
        let conn = get_pool()?;
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
    use crate::test::test::TestEnv;

    const TEST_DATA_DIR: &str = "../test_data/indexer";

    #[test]
    fn test_reset_index() {
        let _env = TestEnv::new();
        Indexer::reset_indexer().unwrap();
    }

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
