
use std::fs;
use std::path::{Path, PathBuf};
use chrono::{DateTime, Local};
use r2d2::PooledConnection;
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::{Connection, params};
use anyhow::{anyhow, Result};

use crate::reader::Item;
use crate::sqlite::get_pool;

#[derive(Debug, PartialEq, Eq)]
pub struct SearchResultDirectory {
    pub name: String,
    pub path: String,
    pub modified_time: String,
}

#[derive(Debug, PartialEq, Eq)]
pub struct SearchResultFile {
    pub name: String,
    pub path: String,
    pub modified_time: String,
}

#[derive(Debug, PartialEq, Eq)]
pub struct SearchResultItem {
    pub page: u64,
    pub line: u64,
    pub content: String,
    pub file: String,
    pub path: String,
}

#[derive(Debug, PartialEq, Eq)]
pub enum SearchResult {
    Directory(SearchResultDirectory),
    File(SearchResultFile),
    Item(SearchResultItem),
}

pub struct Indexer {
    conn: PooledConnection<SqliteConnectionManager>,
}


impl Indexer {

    fn check_indexer_init() -> Result<()> {
        let conn = get_pool()?;
        let row = conn.query_one("select version from indexer_version", [], |row|
            row.get::<_, String>(0)
        ).map_err(|e| anyhow!("Indexer not initialized: {}", e))?;

        if row != "0.1" {
            return Err(anyhow!("Indexer version mismatch: expected 0.1, found {}", row));
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
                page INTEGER NOT NULL,
                line INTEGER NOT NULL,
                content TEXT NOT NULL
            );
            DROP TABLE IF EXISTS indexer_version;
            CREATE TABLE indexer_version (
                version TEXT
            );
            INSERT INTO indexer_version (version) VALUES ('0.1');
        ")?;
        Ok(())
    }
    
    pub fn new() -> Result<Indexer> {
        if let Err(_) = Self::check_indexer_init() {
            Self::reset_indexer()?;
        }
        // println!("Opening index at: {:?}", index_path);
        let conn = get_pool()?;
        // println!("is_autocommit: {}", conn.is_autocommit());
        Ok(Indexer { conn })
    }

    fn check_is_absolute(&self, path: &Path) -> Result<()> {
        if !path.is_absolute() {
            return Err(anyhow!("Path {} is not an absolute path", path.display()));
        }
        Ok(())
    }

    pub fn write_directory(&self, directory: &Path) -> Result<()> {
        self.check_is_absolute(directory)?;
        let dir_name = directory.file_name().unwrap().to_str().unwrap();
        let dir_path = directory.to_str().unwrap();
        let modified_datetime: DateTime<Local> = DateTime::from(fs::metadata(dir_path)?.modified()?);
        let modified_time = modified_datetime.to_rfc3339();

        self.conn.execute(
            "INSERT INTO directories (name, path, modified_time) VALUES (?1, ?2, ?3) ON CONFLICT(path) DO NOTHING",
            params![&dir_name, &dir_path, &modified_time],
        ).unwrap();
        Ok(())
    }

    pub fn get_directory(&self, directory: &Path) -> Result<SearchResultDirectory> {
        self.check_is_absolute(directory)?;
        let dir_path = directory.to_str().unwrap();
        let mut stmt = self.conn.prepare("SELECT name, path, modified_time FROM directories WHERE path = ?1")?;
        let row = stmt.query_row(params![dir_path], |row| {
            Ok(SearchResultDirectory {
                name: row.get(0)?,
                path: row.get(1)?,
                modified_time: row.get(2)?,
            })
        })?;
        Ok(row)
    }

    pub fn write_file_items(&self, file: &Path, items: Vec<Item>) -> Result<()> {
        self.check_is_absolute(file)?;
        let file = file.canonicalize().unwrap();
        let directory_path = file.parent().unwrap().to_str().unwrap();
        let directory_id: i64 = self.conn.query_row(
            "SELECT id FROM directories WHERE path = ?1",
            params![&directory_path],
            |row| row.get(0),
        ).unwrap();
        // println!("write_file_items Directory ID: {}", directory_id);
        
        let file_name = file.file_name().unwrap().to_str().unwrap();
        let modified_datetime: DateTime<Local> = DateTime::from(fs::metadata(&file)?.modified()?);
        let modified_time = modified_datetime.to_rfc3339();

        let file_id: i64 = self.conn.query_row(
            "INSERT INTO files (directory_id, name, modified_time) VALUES (?1, ?2, ?3) ON CONFLICT(directory_id, name) DO NOTHING RETURNING id",
            params![&directory_id, file_name, &modified_time],
            |row| row.get(0),
        ).unwrap();
        // println!("write_file_items File ID: {}", file_id);

        for chunk in items.chunks(1000) {
            let mut query = String::from(
                "INSERT INTO items (file_id, page, line, content) VALUES ",
            );

            // 构建 VALUES 部分 (?, ?, ?, ?), (?, ?, ?, ?), ...
            let values: Vec<String> = (0..chunk.len())
                .map(|i| {
                    let base = i * 4 + 1; // 每个 item 有 4 个参数
                    format!("(?{}, ?{}, ?{}, ?{})", base, base + 1, base + 2, base + 3)
                })
                .collect();
            query.push_str(&values.join(", "));

            // 准备所有参数
            let mut params = Vec::new();
            for item in chunk.iter() {
                params.push(&file_id as &dyn rusqlite::ToSql);
                params.push(&item.page as &dyn rusqlite::ToSql);
                params.push(&item.line as &dyn rusqlite::ToSql);
                params.push(&item.content as &dyn rusqlite::ToSql);
            }

            // 执行批量插入
            self.conn.execute(&query, params.as_slice())?;
        }
        Ok(())
    }

    pub fn get_sub_directories_and_files(&self, directory: &Path) -> Result<(Vec<SearchResultDirectory>, Vec<SearchResultFile>)> {
        self.check_is_absolute(directory)?;

        let mut dirs = Vec::new();
        let mut files = Vec::new();

        let dir_path = directory.to_str().unwrap();
        let mut stmt = self.conn.prepare("SELECT name, path, modified_time FROM directories WHERE path != ?1 AND path LIKE ?2")?;
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

        let mut stmt = self.conn.prepare(r"SELECT files.name, directories.path, files.modified_time 
            FROM files
            JOIN directories
            ON files.directory_id = directories.id
            WHERE directories.path = ?1")?;
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

    pub fn search(&self, content: &str, limit: usize) -> Result<Vec<SearchResult>> {
        let mut result = Vec::new();

        let sql = format!("SELECT name, path, modified_time FROM directories WHERE name LIKE '%{}%' LIMIT {}", content, limit);
        // println!("SQL for directory search: {}", sql);
        let mut stmt = self.conn.prepare(&sql)?;
        let rows = stmt.query_map([], |row| {
            Ok(SearchResultDirectory {
                name: row.get(0)?,
                path: row.get(1)?,
                modified_time: row.get(2)?,
            })
        })?;

        for row in rows {
            result.push(SearchResult::Directory(row.unwrap()));
        }
        // println!("directories result: {:?}", result);

        let sql = format!(r"SELECT files.name, directories.path, files.modified_time
            FROM files
            left outer join directories
            on files.directory_id = directories.id
            WHERE files.name LIKE '%{}%' LIMIT {}", content, limit);
        // println!("SQL for file search: {}", sql);
        let mut stmt = self.conn.prepare(&sql)?;
        let rows = stmt.query_map([], |row| {
            Ok(SearchResultFile {
                name: row.get(0)?,
                path: row.get(1)?,
                modified_time: row.get(2)?,
            })
        })?;

        for row in rows {
            result.push(SearchResult::File(row.unwrap()));
        }
        // println!("files result: {:?}", result);

        let sql = format!(r"SELECT items.page, items.line, items.content, files.name, directories.path
            FROM items
            LEFT OUTER JOIN files ON items.file_id = files.id
            LEFT OUTER JOIN directories ON files.directory_id = directories.id
            WHERE items.content LIKE '%{}%' LIMIT {}", content, limit);
        // println!("SQL for item search: {}", sql);
        let mut stmt = self.conn.prepare(&sql)?;
        let rows = stmt.query_map([], |row| {
            Ok(SearchResultItem {
                page: row.get(0)?,
                line: row.get(1)?,
                content: row.get(2)?,
                file: row.get(3)?,
                path: row.get(4)?,
            })
        })?;

        for row in rows {
            result.push(SearchResult::Item(row.unwrap()));
        }
        // println!("items result: {:?}", result);

        Ok(result)
    }

    pub fn delete_file(&self, file: &Path) -> Result<()> {
        self.check_is_absolute(file)?;
        let file_name = file.file_name().unwrap().to_str().unwrap();
        let directory_path = file.parent().unwrap().to_str().unwrap();

        let file_id: i64 = self.conn.query_row(
            "SELECT id FROM files WHERE name = ?1 and directory_id in (SELECT id FROM directories WHERE path = ?2)",
            params![file_name, &directory_path],
            |row| row.get(0),
        ).unwrap();

        self.conn.execute(
            "DELETE FROM items WHERE file_id = ?1",
            &[&file_id.to_string()],
        ).unwrap();

        self.conn.execute(
            "DELETE FROM files WHERE id = ?1",
            &[&file_id.to_string()],
        ).unwrap();

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
        self.conn.execute(
            "DELETE FROM directories WHERE path = ?1",
            params![dir_path],
        )?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test::test::TestEnv;

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
        let path = Path::new("../test_data/").canonicalize().unwrap();
        indexer.write_directory(&path).unwrap();
    }
    
    #[test]
    fn test_get_directory() {
        let _env = TestEnv::new();
        let indexer = Indexer::new().unwrap();
        let path = Path::new("../test_data/").canonicalize().unwrap();
        indexer.write_directory(&path).unwrap();

        let dir = indexer.get_directory(&path).unwrap();
        assert_eq!(dir.name, "test_data");
        assert_eq!(dir.path, path.canonicalize().unwrap().to_str().unwrap());
    }

    #[test]
    fn test_write_file_items() {
        let _env = TestEnv::new();
        let indexer = Indexer::new().unwrap();

        let file = Path::new("../test_data/1.txt").canonicalize().unwrap();
        indexer.write_directory(file.parent().unwrap()).unwrap();

        let items = vec![
            Item { page: 0, line: 1, content: "Hello, world!".into() },
            Item { page: 0, line: 2, content: "This is a test.".into() },
        ];
        indexer.write_file_items(&file, items).unwrap();
    }

    #[test]
    fn test_get_sub_directories_and_files() {
        let _env = TestEnv::new();
        let indexer = Indexer::new().unwrap();

        let file = Path::new("../test_data/1.txt").canonicalize().unwrap();
        indexer.write_directory(file.parent().unwrap()).unwrap();

        let items = vec![
            Item { page: 0, line: 1, content: "Hello, world!".into() },
            Item { page: 0, line: 2, content: "This is a test.".into() },
        ];
        indexer.write_file_items(&file, items).unwrap();

        let sub_dir_path = Path::new("../test_data/office/").canonicalize().unwrap();
        indexer.write_directory(&sub_dir_path).unwrap();

        let (dir_result, file_result) = indexer.get_sub_directories_and_files(file.parent().unwrap()).unwrap();
        assert_eq!(dir_result.len(), 1);
        assert_eq!(file_result.len(), 1);

        println!("dir_result: {:?}", dir_result);
    }

    #[test]
    fn test_search_path() {
        let _env = TestEnv::new();
        let indexer = Indexer::new().unwrap();
        let file = Path::new("../test_data/").canonicalize().unwrap();
        indexer.write_directory(&file).unwrap();

        let result = indexer.search("test_data", 10).unwrap();
        assert_eq!(result.len(), 1);
        match &result[0] {
            SearchResult::Directory(dir) => {
                assert_eq!(dir.name, "test_data");
                assert_eq!(dir.path, file.canonicalize().unwrap().to_str().unwrap());
            }
            _ => panic!("Expected directory result"),
        }
    }

    #[test]
    fn test_search_file() {
        let _env = TestEnv::new();
        let indexer = Indexer::new().unwrap();
        let items = vec![
            Item { page: 0, line: 1, content: "Hello, world!".into() },
            Item { page: 0, line: 2, content: "This is a test.".into() },
        ];
        let file = Path::new("../test_data/1.txt").canonicalize().unwrap();
        indexer.write_directory(file.parent().unwrap()).unwrap();
        indexer.write_file_items(&file, items).unwrap();

        let result = indexer.search("1.t", 10).unwrap();
        assert_eq!(result.len(), 1);
        match &result[0] {
            SearchResult::File(f) => {
                assert_eq!(f.name, "1.txt");
                assert_eq!(f.path, file.parent().unwrap().canonicalize().unwrap().to_str().unwrap());
            }
            _ => panic!("Expected file result"),
        }
    }

    #[test]
    fn test_search_item() {
        let _env = TestEnv::new();
        let indexer = Indexer::new().unwrap();
        let items = vec![
            Item { page: 0, line: 1, content: "Hello, world!".into() },
            Item { page: 0, line: 2, content: "This is a test.".into() },
        ];
        let file = Path::new("../test_data/1.txt").canonicalize().unwrap();
        indexer.write_directory(file.parent().unwrap()).unwrap();
        indexer.write_file_items(&file, items).unwrap();

        let result = indexer.search("world", 10).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0], SearchResult::Item(SearchResultItem { page: 0,
            line: 1, 
            content: "Hello, world!".into(),
            file: "1.txt".into(),
            path: file.parent().unwrap().canonicalize().unwrap().to_str().unwrap().into(),
        }));
    }

    #[test]
    fn test_delete_file() {
        let _env = TestEnv::new();
        let indexer = Indexer::new().unwrap();
        let items = vec![
            Item { page: 0, line: 1, content: "Hello, world!".into() },
            Item { page: 0, line: 2, content: "This is a test.".into() },
        ];
        let file = Path::new("../test_data/1.txt").canonicalize().unwrap();
        indexer.write_directory(file.parent().unwrap()).unwrap();
        indexer.write_file_items(&file, items).unwrap();

        indexer.delete_file(&file).unwrap();

        let (dir_result, file_result) = indexer.get_sub_directories_and_files(file.parent().unwrap()).unwrap();
        assert_eq!(dir_result.len(), 0);
        assert_eq!(file_result.len(), 0);
    }

    #[test]
    fn test_delete_directory() {
        let _env = TestEnv::new();
        let indexer = Indexer::new().unwrap();
        let items = vec![
            Item { page: 0, line: 1, content: "Hello, world!".into() },
            Item { page: 0, line: 2, content: "This is a test.".into() },
        ];
        let file = Path::new("../test_data/1.txt").canonicalize().unwrap();
        indexer.write_directory(file.parent().unwrap()).unwrap();
        indexer.write_file_items(&file, items).unwrap();
        indexer.write_directory(&Path::new("../test_data/office/").canonicalize().unwrap()).unwrap();

        indexer.delete_directory(file.parent().unwrap()).unwrap();

        let (dir_result, file_result) = indexer.get_sub_directories_and_files(file.parent().unwrap()).unwrap();
        assert_eq!(dir_result.len(), 0);
        assert_eq!(file_result.len(), 0);
    }

}
