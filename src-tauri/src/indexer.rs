
use std::borrow::Cow;
use std::path::{Path, PathBuf};

use tantivy::collector::TopDocs;
use tantivy::query::QueryParser;
use tantivy::schema::*;
use tantivy::{doc, Index, IndexWriter, ReloadPolicy};
use lazy_static::lazy_static;

use crate::reader::Item;

lazy_static! {
    pub static ref TANTIVY_SCHEMA: Schema = {
        let mut schema_builder = Schema::builder();
        schema_builder.add_text_field("path", TEXT | STORED);
        schema_builder.add_text_field("filename", TEXT | STORED);
        schema_builder.add_u64_field("page", STORED);
        schema_builder.add_u64_field("line", STORED);
        schema_builder.add_text_field("content", TEXT | STORED);
        schema_builder.build()
    };

    pub static ref PATH_FIELD: Field = TANTIVY_SCHEMA.get_field("path").unwrap();
    pub static ref FILENAME_FIELD: Field = TANTIVY_SCHEMA.get_field("filename").unwrap();
    pub static ref PAGE_FIELD: Field = TANTIVY_SCHEMA.get_field("page").unwrap();
    pub static ref LINE_FIELD: Field = TANTIVY_SCHEMA.get_field("line").unwrap();
    pub static ref CONTENT_FIELD: Field = TANTIVY_SCHEMA.get_field("content").unwrap();

}

pub struct Indexer {
    index: Index,
}

impl Indexer {

    pub fn init_indexer(path: &Path) -> Result<Indexer, Box<dyn std::error::Error>> {
        let mut schema_builder = Schema::builder();
        let index = Index::create_in_dir(path, TANTIVY_SCHEMA.clone())?;
        Ok(Indexer { index })
    }

    pub fn get_indexer(path: &Path) -> Result<Indexer, Box<dyn std::error::Error>> {
        Ok(Indexer { index: Index::open_in_dir(path)? })
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

    #[test]
    fn test_init_index() {
        let temp_dir = tempdir().unwrap();
        let indexer = Indexer::init_indexer(temp_dir.path()).unwrap();
        assert_eq!(indexer.index.schema().fields().count(), 5);
    }

    #[test]
    fn test_get_index() {
        let temp_dir = tempdir().unwrap();
        let indexer = Indexer::init_indexer(temp_dir.path()).unwrap();
        let opened_indexer = Indexer::get_indexer(temp_dir.path()).unwrap();
        assert_eq!(indexer.index.schema(), opened_indexer.index.schema());
    }

    #[test]
    fn test_write_items() {
        let temp_dir = tempdir().unwrap();
        let _ = Indexer::init_indexer(temp_dir.path());
        let indexer = Indexer::get_indexer(temp_dir.path()).unwrap();
        let items = vec![
            Item { file: Cow::Owned(PathBuf::from("./path/to/file/english_part.txt")), page: 0, line: 1, content: "Hello, world!".into() },
            Item { file: Cow::Owned(PathBuf::from("./path/to/file/english_part.txt")), page: 0, line: 2, content: "This is a test.".into() },
        ];
        indexer.write_items(Path::new("./path/to/file/english_part.txt"), items).unwrap();
    }


    #[test]
    fn test_search_items() {
        let temp_dir = tempdir().unwrap();
        let _ = Indexer::init_indexer(temp_dir.path());
        let indexer = Indexer::get_indexer(temp_dir.path()).unwrap();
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
