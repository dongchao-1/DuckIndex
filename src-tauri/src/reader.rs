use std::{borrow::Cow, path::Path};
use std::collections::HashMap;
use std::vec;
use std::fs::File;
use std::io::{BufRead, BufReader};

#[derive(Debug)]
pub struct Item<'a> {
    pub file: Cow<'a, Path>,
    pub page: u64,
    pub line: u64,
    pub content: String,
}

pub trait Reader {
    fn read<'a>(&self, file_path: &'a Path) -> Result<Vec<Item<'a>>, Box<dyn std::error::Error>>;
    fn supports(&self) -> &str;
}

pub struct CompositeReader {
    reader_map: HashMap<String, Box<dyn Reader>>,
}

impl CompositeReader {
    pub fn new() -> Self {
        let readers: Vec<Box<dyn Reader>> = vec![Box::new(TxtReader)];
        let mut reader_map: HashMap<String, Box<dyn Reader>> = HashMap::new();
        for reader in readers {
            reader_map.insert(
                reader.supports().to_string(),
                reader,
            );
        }
        CompositeReader { reader_map }
    }

    pub fn read<'a>(&self, file_path: &'a Path) -> Result<Vec<Item<'a>>, Box<dyn std::error::Error>> {
        if let Some(extension) = file_path.extension() {
            if let Some(ext_str) = extension.to_str() {
                if let Some(reader) = self.reader_map.get(ext_str) {
                    return reader.read(file_path);
                }
            }
        }
        Ok(vec![])
    }
}

struct TxtReader;
impl Reader for TxtReader {
    fn read<'a>(&self, file_path: &'a Path) -> Result<Vec<Item<'a>>, Box<dyn std::error::Error>> {
        let file = File::open(file_path)?;
        let reader = BufReader::new(file);
        let mut items = vec![];

        for (line_number, line) in reader.lines().enumerate() {
            let line = line?;
            items.push(Item {
                file: Cow::Borrowed(file_path),
                page: 0,
                line: line_number as u64 + 1,
                content: line,
            });
        }
        Ok(items)
    }

    fn supports(&self) -> &str {
        "txt"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_composite_reader() {
        let reader = CompositeReader::new();
        let items = reader.read(Path::new("../test_data/1.txt")).unwrap();
        assert_eq!(items.len(), 4);
    }

    #[test]
    fn test_composite_unknown_extension() {
        let reader = CompositeReader::new();
        let items = reader.read(Path::new("../test_data/1.xyz")).unwrap();
        assert_eq!(items.len(), 0);
    }

    #[test]
    fn test_txt_reader() {
        let reader = TxtReader;
        assert_eq!(reader.supports(), "txt");
        let items = reader.read(Path::new("../test_data/1.txt")).unwrap();
        assert_eq!(items.len(), 4);
    }
}

