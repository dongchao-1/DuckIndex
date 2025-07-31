#[cfg(test)]
mod test{
    use std::borrow::Cow;
    use std::path::{Path, PathBuf};

    use tempfile::tempdir;
    use crate::indexer::Indexer;
    use crate::reader::Item;

    pub fn init() {
        let temp_dir = tempdir().unwrap();
        let _ = Indexer::init_indexer(temp_dir.path());
        let indexer = Indexer::get_indexer(temp_dir.path()).unwrap();
        let items = vec![
            Item { file: Cow::Owned(PathBuf::from("./path/to/file/english_part.txt")), page: 0, line: 1, content: "Hello, world!".into() },
            Item { file: Cow::Owned(PathBuf::from("./path/to/file/english_part.txt")), page: 0, line: 2, content: "This is a test.".into() },
        ];
        indexer.write_items(Path::new("./path/to/file/english_part.txt"), items).unwrap();
    }
}
