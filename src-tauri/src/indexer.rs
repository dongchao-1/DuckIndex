
use std::borrow::Cow;
use std::path::{Path, PathBuf};

use tantivy::collector::TopDocs;
use tantivy::query::QueryParser;
use tantivy::schema::*;
use tantivy::{doc, Index, IndexWriter, ReloadPolicy};
use once_cell::sync::Lazy;

use crate::reader::Item;
use crate::CONFIG;

// 全局 Schema 实例
pub static TANTIVY_SCHEMA: Lazy<Schema> = Lazy::new(|| {
    let mut schema_builder = Schema::builder();
    schema_builder.add_text_field("path", TEXT | STORED);
    schema_builder.add_text_field("filename", TEXT | STORED);
    schema_builder.add_u64_field("page", STORED);
    schema_builder.add_u64_field("line", STORED);
    schema_builder.add_text_field("content", TEXT | STORED);
    schema_builder.build()
});

// 提取字段
pub static PATH_FIELD: Lazy<Field> = Lazy::new(|| {
    TANTIVY_SCHEMA.get_field("path").unwrap()
});
pub static FILENAME_FIELD: Lazy<Field> = Lazy::new(|| {
    TANTIVY_SCHEMA.get_field("filename").unwrap()
});
pub static PAGE_FIELD: Lazy<Field> = Lazy::new(|| {
    TANTIVY_SCHEMA.get_field("page").unwrap()
});
pub static LINE_FIELD: Lazy<Field> = Lazy::new(|| {
    TANTIVY_SCHEMA.get_field("line").unwrap()
});
pub static CONTENT_FIELD: Lazy<Field> = Lazy::new(|| {
    TANTIVY_SCHEMA.get_field("content").unwrap()
});

pub struct Indexer {
    index: Index,
}

impl Indexer {

    fn get_index_path() -> PathBuf {
        PathBuf::from(CONFIG.get().unwrap().index_path.clone())
    }
    
    pub fn init_indexer() -> Result<Indexer, Box<dyn std::error::Error>> {
        let index_path = Self::get_index_path();
        let index = Index::create_in_dir(index_path, TANTIVY_SCHEMA.clone())?;
        Ok(Indexer { index })
    }

    pub fn reset_indexer() -> Result<(), Box<dyn std::error::Error>> {
        let index_path = Self::get_index_path();
        if index_path.exists() {
            std::fs::remove_dir_all(index_path)?;
        }
        Self::init_indexer()?;
        Ok(())
    }
    
    pub fn get_indexer() -> Result<Indexer, Box<dyn std::error::Error>> {
        let index_path = Self::get_index_path();
        Ok(Indexer { index: Index::open_in_dir(index_path)? })
    }

    pub fn write_items(&self, file: &Path, items: Vec<Item>) -> Result<(), Box<dyn std::error::Error>> {
        let mut index_writer: IndexWriter = self.index.writer(50_000_000).unwrap();

        let path_str = file.parent().unwrap().to_str().unwrap();
        let filename_str = file.file_name().unwrap().to_str().unwrap();
        for item in items {
            let doc = doc!(
                PATH_FIELD.clone() => path_str,
                FILENAME_FIELD.clone() => filename_str,
                PAGE_FIELD.clone() => item.page as u64,
                LINE_FIELD.clone() => item.line as u64,
                CONTENT_FIELD.clone() => item.content.as_str()
            );
            index_writer.add_document(doc)?;
        }
        index_writer.commit().unwrap();
        Ok(())
    }

    pub fn search(&self, content: &str, limit: usize) -> Result<Vec<Item>, Box<dyn std::error::Error>> {
        let reader = self.index
            .reader_builder()
            .reload_policy(ReloadPolicy::OnCommitWithDelay)
            .try_into()?;
        let searcher = reader.searcher();
        let query_parser = QueryParser::for_index(&self.index, vec![PATH_FIELD.clone(), FILENAME_FIELD.clone(), CONTENT_FIELD.clone()]);

        let query = query_parser.parse_query(content)?;

        let top_docs = searcher.search(&query, &TopDocs::with_limit(limit))?;
        let mut results = Vec::new();
        for (_score, doc_address) in top_docs {
            let retrieved_doc: TantivyDocument = searcher.doc(doc_address)?;
            let path_str = retrieved_doc.get_first(PATH_FIELD.clone()).unwrap().as_str().unwrap();
            let filename_str = retrieved_doc.get_first(FILENAME_FIELD.clone()).unwrap().as_str().unwrap();
            let mut file = PathBuf::from(path_str);
            file.push(filename_str);

            results.push(Item {
                file: Cow::Owned(file),
                page: retrieved_doc.get_first(PAGE_FIELD.clone()).unwrap().as_u64().unwrap(),
                line: retrieved_doc.get_first(LINE_FIELD.clone()).unwrap().as_u64().unwrap(),
                content: retrieved_doc.get_first(CONTENT_FIELD.clone()).unwrap().as_str().unwrap().to_string(),
            });
        }
        Ok(results)
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    use crate::test::test::TestEnv;

    #[test]
    fn test_init_index() {
        let _env = TestEnv::new();
        let indexer = Indexer::init_indexer().unwrap();
        assert_eq!(indexer.index.schema().fields().count(), 5);
    }

    #[test]
    fn test_get_index() {
        let _env = TestEnv::new();
        let indexer = Indexer::init_indexer().unwrap();
        let opened_indexer = Indexer::get_indexer().unwrap();
        assert_eq!(indexer.index.schema(), opened_indexer.index.schema());
    }

    #[test]
    fn test_write_items() {
        let _env = TestEnv::new();
        let _ = Indexer::init_indexer();
        let indexer = Indexer::get_indexer().unwrap();
        let items = vec![
            Item { file: Cow::Owned(PathBuf::from("./path/to/file/english_part.txt")), page: 0, line: 1, content: "Hello, world!".into() },
            Item { file: Cow::Owned(PathBuf::from("./path/to/file/english_part.txt")), page: 0, line: 2, content: "This is a test.".into() },
        ];
        indexer.write_items(Path::new("./path/to/file/english_part.txt"), items).unwrap();
    }


    #[test]
    fn test_search_items() {
        let _env = TestEnv::new();
        let _ = Indexer::init_indexer();
        let indexer = Indexer::get_indexer().unwrap();
        let items = vec![
            Item { file: Cow::Owned(PathBuf::from("./path/to/file/english_part.txt")), page: 0, line: 1, content: "Hello, world!".into() },
            Item { file: Cow::Owned(PathBuf::from("./path/to/file/english_part.txt")), page: 0, line: 2, content: "This is a test.".into() },
        ];
        indexer.write_items(Path::new("./path/to/file/english_part.txt"), items).unwrap();
        let result = indexer.search("is", 10).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].content, "This is a test.");
        // println!("Search result: {:?}", result);
    }
}
