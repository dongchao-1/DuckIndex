use std::{borrow::Cow, path::Path};
use std::collections::HashMap;
use std::vec;
use std::fs::File;
use std::io::{BufRead, BufReader};
use tempfile::TempDir;
use zip::ZipArchive;
use quick_xml::events::Event as quickXmlEvent;
use quick_xml::Reader as quickXmlReader;
use lopdf::{Document as pdfDocument, Object as pdfObject};

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
        let readers: Vec<Box<dyn Reader>> = vec![Box::new(TxtReader), Box::new(DocxReader)];
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


struct DocxReader;
impl Reader for DocxReader {
    fn read<'a>(&self, file_path: &'a Path) -> Result<Vec<Item<'a>>, Box<dyn std::error::Error>> {
        let temp_dir = TempDir::new()?;
        let file = File::open(file_path)?;
        let mut archive = ZipArchive::new(file)?;
        archive.extract(&temp_dir)?;


        // 提取 document.xml
        let document_path = temp_dir.path().join("word/document.xml");
        let reader = BufReader::new(File::open(document_path)?);
        let mut xml_reader = quickXmlReader::from_reader(reader);

        let mut txt = String::new();
        let mut buf = Vec::new();
        let mut items = vec![];
        let mut line = 1;

        loop {
            match xml_reader.read_event_into(&mut buf)? {
                quickXmlEvent::Start(e) if e.name().as_ref() == b"w:p" => {
                    if let Some(item) = self.create_item(file_path, &mut txt, &mut line) {
                        items.push(item);
                    }
                }
                quickXmlEvent::Text(e) => {
                    txt.push_str(&e.decode()?);
                }
                quickXmlEvent::Eof => {
                    if let Some(item) = self.create_item(file_path, &mut txt, &mut line) {
                        items.push(item);
                    }
                    break;
                }, // 文件结束
                _ => (),
            }
            buf.clear();
        }

        Ok(items)
    }

    fn supports(&self) -> &str {
        "docx"
    }
}

impl DocxReader {
    fn create_item<'a>(&self, file_path: &'a Path, txt: &mut String, line: &mut u64) -> Option<Item<'a>> {
        let item = if !txt.trim().is_empty() {
            let txt_ret = txt.trim().to_string();
            txt.clear();
            let line_ret = *line;
            *line += 1;
            Some(Item {
                file: Cow::Borrowed(file_path),
                page: 0,
                line: line_ret,
                content: txt_ret,
            })
        } else {
            None
        };

        item
    }
}


struct PdfReader;
impl Reader for PdfReader {
    fn read<'a>(&self, file_path: &'a Path) -> Result<Vec<Item<'a>>, Box<dyn std::error::Error>> {
        let mut items = vec![];
        let doc = pdfDocument::load(file_path)?;
        let mut text = String::new();

        for page_num in 1..=doc.get_pages().len() {
            match doc.extract_text(&[page_num.try_into().unwrap()]) {
                Ok(page_text) => {
                    println!("page_text: {}", page_text);
                    text.push_str(&page_text.strip_suffix("\n").unwrap());
                }
                Err(_) => {
                    // You may want to handle the error, log it, or skip the page
                    continue;
                }
            }
        }
        let lines = text.lines().collect::<Vec<_>>();
        let mut result = String::new();

        for (i, line) in lines.iter().enumerate() {
            result.push_str(line);
            if i < lines.len() - 1 { // 不是最后一行
                if line.chars().last().map_or(false, |c| c.is_ascii_alphabetic()) {
                    result.push(' '); // 英文行尾加空格
                }
            }
        }

        // println!("Extracted text: {}", text);
        // println!("result: {}", result);
        items.push(Item {
            file: Cow::Borrowed(file_path),
            page: 0,
            line: 1,
            content: result,
        });
        Ok(items)
    }

    fn supports(&self) -> &str {
        "pdf"
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

    #[test]
    fn test_docx_reader() {
        let reader = DocxReader;
        assert_eq!(reader.supports(), "docx");
        let items = reader.read(Path::new("../test_data/test.docx")).unwrap();
        // println!("Items: {:?}", items);
        assert_eq!(items.len(), 10);
    }

    #[test]
    fn test_pdf_reader() {
        let reader = PdfReader;
        assert_eq!(reader.supports(), "pdf");
        let items = reader.read(Path::new("../test_data/test.pdf")).unwrap();
        // println!("Items: {:?}", items);
        assert_eq!(items.len(), 1);
    }
}

