use std::sync::Arc;
use std::path::Path;
use std::collections::HashMap;
use std::{fs, vec};
use std::fs::File;
use std::io::{BufRead, BufReader};
use tempfile::TempDir;
use zip::ZipArchive;
use quick_xml::events::Event as quickXmlEvent;
use quick_xml::Reader as quickXmlReader;
use lopdf::Document as pdfDocument;


#[derive(Debug)]
pub struct Item {
    pub page: u64,
    pub line: u64,
    pub content: String,
}


pub trait Reader {
    fn read(&self, file_path: &Path) -> Result<Vec<Item>, Box<dyn std::error::Error>>;
    fn supports(&self) -> Vec<&str>;
}

pub struct CompositeReader {
    reader_map: HashMap<String, Arc<dyn Reader>>,
}

impl CompositeReader {
    pub fn new() -> Self {
        let readers: Vec<Arc<dyn Reader>> = vec![Arc::new(TxtReader), Arc::new(DocxReader), Arc::new(PdfReader), Arc::new(PptxReader)];
        let mut reader_map: HashMap<String, Arc<dyn Reader>> = HashMap::new();
        for reader in readers {
            for ext in reader.supports() {
                reader_map.insert(ext.to_string(), reader.clone());
            }
        }
        CompositeReader { reader_map }
    }

    pub fn supports(&self) -> Vec<String> {
        self.reader_map.keys().cloned().collect()
    }
    
    pub fn read(&self, file_path: &Path) -> Result<Vec<Item>, Box<dyn std::error::Error>> {
        if let Some(extension) = file_path.extension() {
            if let Some(ext_str) = extension.to_str() {
                let ext_str = ext_str.to_lowercase();
                if let Some(reader) = self.reader_map.get(&ext_str) {
                    return reader.read(file_path);
                }
            }
        }
        Err("Unsupported file type".into())
    }
}

struct TxtReader;
impl Reader for TxtReader {
    fn read(&self, file_path: &Path) -> Result<Vec<Item>, Box<dyn std::error::Error>> {
        let file = File::open(file_path)?;
        let reader = BufReader::new(file);
        let mut items = vec![];

        for (line_number, line) in reader.lines().enumerate() {
            let line = line?;
            items.push(Item {
                page: 0,
                line: line_number as u64 + 1,
                content: line,
            });
        }
        Ok(items)
    }

    fn supports(&self) -> Vec<&str> {
        vec!["txt", "md", "markdown"]
    }
}


struct DocxReader;
impl Reader for DocxReader {
    fn read(&self, file_path: &Path) -> Result<Vec<Item>, Box<dyn std::error::Error>> {
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
                    if let Some(item) = self.create_item(&mut txt, &mut line) {
                        items.push(item);
                    }
                }
                quickXmlEvent::Text(e) => {
                    txt.push_str(&e.decode()?);
                }
                quickXmlEvent::Eof => {
                    if let Some(item) = self.create_item(&mut txt, &mut line) {
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

    fn supports(&self) -> Vec<&str> {
        vec!["docx"]
    }
}

impl DocxReader {
    fn create_item(&self, txt: &mut String, line: &mut u32) -> Option<Item> {
        let item = if !txt.trim().is_empty() {
            let txt_ret = txt.trim().to_string();
            txt.clear();
            let line_ret = *line;
            *line += 1;
            Some(Item {
                page: 0,
                line: line_ret as u64,
                content: txt_ret,
            })
        } else {
            None
        };

        item
    }
}


struct PptxReader;
impl Reader for PptxReader {
    fn read(&self, file_path: &Path) -> Result<Vec<Item>, Box<dyn std::error::Error>> {
        let temp_dir = TempDir::new()?;
        let file = File::open(file_path)?;
        let mut archive = ZipArchive::new(file)?;
        archive.extract(&temp_dir)?;

        let document_path = temp_dir.path().join("ppt/slides/");
        let mut txt = String::new();
        let mut buf = Vec::new();
        let mut items = vec![];

        if let Ok(entries) = fs::read_dir(Path::new(&document_path)) {
            for entry in entries {
                if let Ok(entry) = entry {
                    let file_name = entry.file_name();
                    let file_name = file_name.to_string_lossy();

                    if file_name.starts_with("slide") && file_name.ends_with(".xml") {
                        if let Some(number_str) = file_name
                            .strip_prefix("slide")
                            .and_then(|s| s.strip_suffix(".xml")) {
                            if let Ok(page_number) = number_str.parse::<u64>() {
                                let reader = BufReader::new(File::open(entry.path())?);
                                let mut xml_reader = quickXmlReader::from_reader(reader);

                                loop {
                                    match xml_reader.read_event_into(&mut buf)? {
                                        quickXmlEvent::Start(e) if e.name().as_ref() == b"a:p" => {
                                            if let Some(item) = self.create_item(&mut txt, page_number) {
                                                items.push(item);
                                            }
                                        }
                                        quickXmlEvent::Text(e) => {
                                            txt.push_str(&e.decode()?);
                                        }
                                        quickXmlEvent::Eof => {
                                            if let Some(item) = self.create_item(&mut txt, page_number) {
                                                items.push(item);
                                            }
                                            break;
                                        }, // 文件结束
                                        _ => (),
                                    }
                                    buf.clear();
                                }
                            }
                        }
                    }
                }
            }
        }
        Ok(items)
    }

    fn supports(&self) -> Vec<&str> {
        vec!["pptx"]
    }
}

impl PptxReader {
    fn create_item(&self, txt: &mut String, page: u64) -> Option<Item> {
        let item = if !txt.trim().is_empty() {
            let txt_ret = txt.trim().to_string();
            txt.clear();
            Some(Item {
                page: page,
                line: 0,
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
    fn read(&self, file_path: &Path) -> Result<Vec<Item>, Box<dyn std::error::Error>> {
        let mut items = vec![];
        let doc = pdfDocument::load(file_path)?;
        let mut text = String::new();

        for page_num in 1..=doc.get_pages().len() {
            match doc.extract_text(&[page_num.try_into().unwrap()]) {
                Ok(page_text) => {
                    // println!("page_text: {}", page_text);
                    text.push_str(&page_text.trim_end_matches("\n"));
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
            page: 0,
            line: 0,
            content: result,
        });
        Ok(items)
    }

    fn supports(&self) -> Vec<&str> {
        vec!["pdf"]
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
        let result = reader.read(Path::new("../test_data/1.xyz"));
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().to_string(), "Unsupported file type");
    }

    #[test]
    fn test_txt_reader() {
        let reader = TxtReader;
        assert_eq!(reader.supports(), vec!["txt", "md", "markdown"]);
        let items = reader.read(Path::new("../test_data/1.txt")).unwrap();
        assert_eq!(items.len(), 4);
    }

    #[test]
    fn test_docx_reader() {
        let reader = DocxReader;
        assert_eq!(reader.supports(), vec!["docx"]);
        let items = reader.read(Path::new("../test_data/office/test.docx")).unwrap();
        // println!("Items: {:?}", items);
        assert_eq!(items.len(), 10);
    }

    #[test]
    fn test_pptx_reader() {
        let reader = PptxReader;
        assert_eq!(reader.supports(), vec!["pptx"]);
        let items = reader.read(Path::new("../test_data/office/test.pptx")).unwrap();
        // println!("Items: {:?}", items);
        assert_eq!(items.len(), 5);
    }

    #[test]
    fn test_pdf_reader() {
        let reader = PdfReader;
        assert_eq!(reader.supports(), vec!["pdf"]);
        let items = reader.read(Path::new("../test_data/test.pdf")).unwrap();
        // println!("Items: {:?}", items);
        assert_eq!(items.len(), 1);
    }
}
