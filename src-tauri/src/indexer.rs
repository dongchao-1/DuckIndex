
use std::path::{Path, PathBuf};

use rusqlite::{Connection, params};

use crate::reader::Item;
use crate::CONFIG;



#[derive(Debug, PartialEq, Eq)]
pub struct SearchResultDirectory {
    pub name: String,
    pub path: String,
}


#[derive(Debug, PartialEq, Eq)]
pub struct SearchResultFile {
    pub name: String,
    pub path: String,
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
    conn: rusqlite::Connection,
}


impl Indexer {

    fn get_index_path() -> PathBuf {
        PathBuf::from(CONFIG.get().unwrap().index_path.clone())
    }

    pub fn reset_indexer() -> Result<(), Box<dyn std::error::Error>> {
        let index_path = Self::get_index_path();
        if index_path.exists() {
            std::fs::remove_dir_all(&index_path)?;
        }
        println!("Creating index directory at: {:?}", index_path);
        std::fs::create_dir_all(&index_path)?;
        let conn = Connection::open(index_path.join("index.db"))?;
        conn.execute_batch(
            r"
            CREATE TABLE directories (
                id INTEGER PRIMARY KEY, 
                name TEXT NOT NULL, 
                path TEXT NOT NULL, 
                UNIQUE (path)
            );
            CREATE TABLE files (
                id INTEGER PRIMARY KEY, 
                directory_id INTEGER NOT NULL, 
                name TEXT NOT NULL, 
                UNIQUE (directory_id, name)
            );
            CREATE TABLE items (
                id INTEGER PRIMARY KEY, 
                file_id INTEGER NOT NULL,
                page INTEGER NOT NULL, 
                line INTEGER NOT NULL, 
                content TEXT NOT NULL
            );
            "
        )?;
        Ok(())
    }
    
    pub fn get_indexer() -> Result<Indexer, Box<dyn std::error::Error>> {
        let index_path = Self::get_index_path();
        if !index_path.exists() {
            Self::reset_indexer()?;
        }
        // println!("Opening index at: {:?}", index_path);
        let conn = Connection::open(index_path.join("index.db"))?;
        // println!("is_autocommit: {}", conn.is_autocommit());
        Ok(Indexer { conn })
    }

    pub fn write_directory(&self, directory: &Path) -> Result<(), Box<dyn std::error::Error>> {
        if !directory.is_dir() {
            return Err(format!("Path {} is not a directory", directory.display()).into());
        }
        let directory = directory.canonicalize().unwrap();
        let dir_name = directory.file_name().unwrap().to_str().unwrap();
        let dir_path = directory.to_str().unwrap();
        self.conn.execute(
            "INSERT INTO directories (name, path) VALUES (?1, ?2) ON CONFLICT(path) DO NOTHING",
            params![&dir_name, &dir_path],
        ).unwrap();
        Ok(())
    }

    pub fn write_file_items(&self, file: &Path, items: Vec<Item>) -> Result<(), Box<dyn std::error::Error>> {
        if !file.is_file() {
            return Err(format!("Path {} is not a file", file.display()).into());
        }
        let file = file.canonicalize().unwrap();
        let directory_path = file.parent().unwrap().to_str().unwrap();
        let directory_id: i64 = self.conn.query_row(
            "SELECT id FROM directories WHERE path = ?1",
            params![&directory_path],
            |row| row.get(0),
        ).unwrap();
        // println!("write_file_items Directory ID: {}", directory_id);
        
        let file_name = file.file_name().unwrap().to_str().unwrap();

        let file_id: i64 = self.conn.query_row(
            "INSERT INTO files (directory_id, name) VALUES (?1, ?2)  ON CONFLICT(directory_id, name) DO NOTHING RETURNING id",
            params![&directory_id, file_name],
            |row| row.get(0),
        ).unwrap();
        // println!("write_file_items File ID: {}", file_id);

        for item in items {
            self.conn.execute(
                "INSERT INTO items (file_id, page, line, content) VALUES (?1, ?2, ?3, ?4)",
                params![&file_id, &item.page, &item.line, &item.content],
            ).unwrap();
            // println!("write_file_items Item inserted: {:?}", item);
        }
        Ok(())
    }

    pub fn search(&self, content: &str, limit: usize) -> Result<Vec<SearchResult>, Box<dyn std::error::Error>> {
        let mut result = Vec::new();

        let sql = format!("SELECT name, path FROM directories WHERE name LIKE '%{}%' LIMIT {}", content, limit);
        // println!("SQL for directory search: {}", sql);
        let mut stmt = self.conn.prepare(&sql)?;
        let rows = stmt.query_map([], |row| {
            Ok(SearchResultDirectory {
                name: row.get(0)?,
                path: row.get(1)?,
            })
        })?;

        for row in rows {
            result.push(SearchResult::Directory(row.unwrap()));
        }
        // println!("directories result: {:?}", result);

        let sql = format!(r"SELECT files.name, directories.path 
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

    pub fn delete_file(&self, file: &Path) -> Result<(), Box<dyn std::error::Error>> {
        // TODO 这时候文件已经删除了，不应该判断了
        // if !file.is_file() {
        //     return Err(format!("Path {} is not a file", file.display()).into());
        // }

        let file = file.canonicalize().unwrap();
        let file_name = file.file_name().unwrap().to_str().unwrap();
        let directory_path = file.parent().unwrap().to_str().unwrap();

        let directory_id: i64 = self.conn.query_row(
            "SELECT id FROM directories WHERE path = ?1",
            &[&directory_path],
            |row| row.get(0),
        ).unwrap();

        let file_id: i64 = self.conn.query_row(
            "SELECT id FROM files WHERE directory_id = ?1 AND name = ?2",
            &[&directory_id.to_string(), file_name],
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
        let opened_indexer = Indexer::get_indexer().unwrap();
    }

    #[test]
    fn test_path_items() {
        let _env = TestEnv::new();
        let path = Path::new("..\\test_data");
        let path = Path::new("\\\\?\\C:\\Users\\dongchao\\Code\\deepindex\\test_data");
        println!("{}", path.to_str().unwrap());
        println!("{}", path.exists());
        println!("{}", path.canonicalize().unwrap().to_str().unwrap());

        println!("{}", path.parent().unwrap().to_str().unwrap());
        println!("{}", path.parent().unwrap().file_name().unwrap().to_str().unwrap());
        println!("{}", path.canonicalize().unwrap().to_str().unwrap());
    }

    
    pub fn print_all_rows(conn: &rusqlite::Connection, table: &str) {
        let sql = format!("SELECT * FROM {}", table);
        let mut stmt = conn.prepare(&sql).unwrap();
        let col_count = stmt.column_count();
        let col_names: Vec<String> = stmt.column_names().into_iter().map(|s| s.to_string()).collect();

        let rows = stmt.query_map([], |row| {
            let mut vals = Vec::new();
            for i in 0..col_count {
                let v: rusqlite::types::Value = row.get(i)?;
                vals.push(format!("{:?}", v));
            }
            Ok(vals)
        }).unwrap();

        println!("===== {} =====", table);
        println!("Columns: {:?}", col_names);
        for r in rows {
            println!("{:?}", r.unwrap());
        }
    }

    #[test]
    fn test_write_directory() {
        let _env = TestEnv::new();
        let indexer = Indexer::get_indexer().unwrap();
        let path = Path::new("../test_data/");
        indexer.write_directory(path).unwrap();
    }

    #[test]
    fn test_write_file_items() {
        let _env = TestEnv::new();
        let indexer = Indexer::get_indexer().unwrap();

        let file = Path::new("../test_data/1.txt");
        indexer.write_directory(file.parent().unwrap()).unwrap();

        let items = vec![
            Item { page: 0, line: 1, content: "Hello, world!".into() },
            Item { page: 0, line: 2, content: "This is a test.".into() },
        ];
        indexer.write_file_items(file, items).unwrap();
    }


    #[test]
    fn test_search_path() {
        let _env = TestEnv::new();
        let indexer = Indexer::get_indexer().unwrap();
        let file = Path::new("../test_data/");
        indexer.write_directory(file).unwrap();

        let result = indexer.search("test_data", 10).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0], SearchResult::Directory(SearchResultDirectory { 
            name: "test_data".into(), 
            path: file.canonicalize().unwrap().to_str().unwrap().into() 
        }));
    }

    
    #[test]
    fn test_search_file() {
        let _env = TestEnv::new();
        let indexer = Indexer::get_indexer().unwrap();
        let items = vec![
            Item { page: 0, line: 1, content: "Hello, world!".into() },
            Item { page: 0, line: 2, content: "This is a test.".into() },
        ];
        let file = Path::new("../test_data/1.txt");
        indexer.write_directory(file.parent().unwrap()).unwrap();
        indexer.write_file_items(file, items).unwrap();

        let result = indexer.search("1.t", 10).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0], SearchResult::File(SearchResultFile { 
            name: "1.txt".into(), 
            path: file.parent().unwrap().canonicalize().unwrap().to_str().unwrap().into() 
        }));
    }

    #[test]
    fn test_search_item() {
        let _env = TestEnv::new();
        let indexer = Indexer::get_indexer().unwrap();
        let items = vec![
            Item { page: 0, line: 1, content: "Hello, world!".into() },
            Item { page: 0, line: 2, content: "This is a test.".into() },
        ];
        let file = Path::new("../test_data/1.txt");
        indexer.write_directory(file.parent().unwrap()).unwrap();
        indexer.write_file_items(file, items).unwrap();

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
        let indexer = Indexer::get_indexer().unwrap();
        let items = vec![
            Item { page: 0, line: 1, content: "Hello, world!".into() },
            Item { page: 0, line: 2, content: "This is a test.".into() },
        ];
        let file = Path::new("../test_data/1.txt");
        indexer.write_directory(file.parent().unwrap()).unwrap();
        indexer.write_file_items(file, items).unwrap();

        indexer.delete_file(file).unwrap();

        let result = indexer.search("1.t", 10).unwrap();
        assert_eq!(result.len(), 0);

        let result = indexer.search("world", 10).unwrap();
        assert_eq!(result.len(), 0);

        let result = indexer.search("test_data", 10).unwrap();
        assert_eq!(result.len(), 1);
    }

}
