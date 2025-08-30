use anyhow::{anyhow, Result};
use lopdf::Document as pdfDocument;
use quick_xml::events::Event as quickXmlEvent;
use quick_xml::Reader as quickXmlReader;
use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;
use std::sync::Arc;
use std::{fs, vec};
use tempfile::TempDir;
use zip::ZipArchive;
use tesseract::Tesseract;

#[derive(Debug)]
pub struct Item {
    pub content: String,
}

pub trait Reader {
    fn read(&self, file_path: &Path) -> Result<Vec<Item>>;
    fn supports(&self) -> Vec<&str>;
}

pub struct CompositeReader {
    reader_map: HashMap<String, Arc<dyn Reader>>,
    supports_ext: HashSet<String>,
}

impl CompositeReader {
    pub fn new() -> Result<Self> {
        let readers: Vec<Arc<dyn Reader>> = vec![
            Arc::new(TxtReader),
            Arc::new(DocxReader),
            Arc::new(PdfReader),
            Arc::new(PptxReader),
            Arc::new(XlsxReader),
            Arc::new(OcrReader),
        ];
        let mut reader_map: HashMap<String, Arc<dyn Reader>> = HashMap::new();
        for reader in readers {
            for ext in reader.supports() {
                reader_map.insert(ext.to_string(), reader.clone());
            }
        }
        let supports_ext = reader_map.keys().cloned().collect();
        Ok(CompositeReader {
            reader_map,
            supports_ext,
        })
    }

    fn is_hidden(&self, path: &Path) -> Result<bool> {
        #[cfg(target_os = "windows")]
        {
            use std::os::windows::fs::MetadataExt;
            let metadata = path.metadata()?;
            let attributes = metadata.file_attributes();
            // FILE_ATTRIBUTE_HIDDEN 的值是 0x2
            Ok((attributes & 0x2) > 0)
        }

        #[cfg(not(target_os = "windows"))]
        {
            if let Some(file_name) = path.file_name() {
                if let Some(s) = file_name.to_str() {
                    // 检查文件名是否以点开头
                    return Ok(s.starts_with('.'));
                }
            }
            Ok(false)
        }
    }

    pub fn supports(&self, file: &Path) -> Result<bool> {
        if self.is_hidden(file)? {
            return Ok(false);
        }
        
        if let Some(ext) = file.extension() {
            let ext_str = ext.to_str().unwrap().to_lowercase();
            return Ok(self.supports_ext.contains(&ext_str));
        }
        Ok(false)
    }

    pub fn read(&self, file_path: &Path) -> Result<Vec<Item>> {
        if let Some(ext) = file_path.extension() {
            let ext_str = ext.to_str().unwrap().to_lowercase();
            if let Some(reader) = self.reader_map.get(&ext_str) {
                return reader.read(file_path);
            }
        }
        Err(anyhow!("Unsupported file type"))
    }
}

struct TxtReader;
impl Reader for TxtReader {
    fn read(&self, file_path: &Path) -> Result<Vec<Item>> {
        let file = File::open(file_path)?;
        let reader = BufReader::new(file);
        let mut items = vec![];

        for line in reader.lines() {
            let line = line?;
            items.push(Item {
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
    fn read(&self, file_path: &Path) -> Result<Vec<Item>> {
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

        loop {
            match xml_reader.read_event_into(&mut buf)? {
                quickXmlEvent::Start(e) if e.name().as_ref() == b"w:p" => {
                    if !txt.trim().is_empty() {
                        items.push(Item {
                            content: txt.trim().to_string(),
                        });
                        txt.clear();
                    }
                }
                quickXmlEvent::Text(e) => {
                    txt.push_str(&e.decode()?);
                }
                quickXmlEvent::Eof => {
                    if !txt.trim().is_empty() {
                        items.push(Item {
                            content: txt.trim().to_string(),
                        });
                    }
                    break;
                } // 文件结束
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

struct PptxReader;
impl Reader for PptxReader {
    fn read(&self, file_path: &Path) -> Result<Vec<Item>> {
        let temp_dir = TempDir::new()?;
        let file = File::open(file_path)?;
        let mut archive = ZipArchive::new(file)?;
        archive.extract(&temp_dir)?;

        let document_path = temp_dir.path().join("ppt/slides/");
        let mut items = vec![];

        for entry in fs::read_dir(Path::new(&document_path))? {
            let entry = entry?;
            let file_name = entry.file_name();
            let file_name = file_name.to_string_lossy();

            if file_name.starts_with("slide") && file_name.ends_with(".xml") {
                let reader = BufReader::new(File::open(entry.path())?);
                let mut xml_reader = quickXmlReader::from_reader(reader);
                let mut txt = String::new();
                let mut buf = Vec::new();
                loop {
                    match xml_reader.read_event_into(&mut buf)? {
                        quickXmlEvent::Start(e) if e.name().as_ref() == b"a:p" => {
                            if !txt.trim().is_empty() {
                                items.push(Item {
                                    content: txt.trim().to_string(),
                                });
                                txt.clear();
                            }
                        }
                        quickXmlEvent::Text(e) => {
                            txt.push_str(&e.decode()?);
                        }
                        quickXmlEvent::Eof => {
                            if !txt.trim().is_empty() {
                                items.push(Item {
                                    content: txt.trim().to_string(),
                                });
                            }
                            break;
                        } // 文件结束
                        _ => (),
                    }
                    buf.clear();
                }
            }
        }
        Ok(items)
    }

    fn supports(&self) -> Vec<&str> {
        vec!["pptx"]
    }
}

struct XlsxReader;
impl Reader for XlsxReader {
    fn read(&self, file_path: &Path) -> Result<Vec<Item>> {
        let temp_dir = TempDir::new()?;
        let file = File::open(file_path)?;
        let mut archive = ZipArchive::new(file)?;
        archive.extract(&temp_dir)?;

        let document_path = temp_dir.path().join("xl/sharedStrings.xml");
        let mut items = vec![];

        let reader = BufReader::new(File::open(document_path)?);
        let mut xml_reader = quickXmlReader::from_reader(reader);
        let mut buf = Vec::new();
        let mut current_text = String::new();
        let mut in_si = false;
        let mut in_text = false;

        loop {
            match xml_reader.read_event_into(&mut buf)? {
                quickXmlEvent::Start(e) => {
                    match e.name().as_ref() {
                        b"si" => {
                            in_si = true;
                            current_text.clear();
                        }
                        b"t" if in_si => {
                            in_text = true;
                        }
                        _ => {}
                    }
                }
                quickXmlEvent::Text(e) if in_text => {
                    current_text.push_str(&e.decode()?);
                }
                quickXmlEvent::End(e) => {
                    match e.name().as_ref() {
                        b"si" => {
                            if in_si && !current_text.trim().is_empty() {
                                items.push(Item {
                                    content: current_text.trim().to_string(),
                                });
                            }
                            in_si = false;
                            current_text.clear();
                        }
                        b"t" => {
                            in_text = false;
                        }
                        _ => {}
                    }
                }
                quickXmlEvent::Eof => break,
                _ => {}
            }
            buf.clear();
        }

        Ok(items)
    }

    fn supports(&self) -> Vec<&str> {
        vec!["xlsx"]
    }
}

struct PdfReader;
impl Reader for PdfReader {
    fn read(&self, file_path: &Path) -> Result<Vec<Item>> {
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
            if i < lines.len() - 1 {
                // 不是最后一行
                if line
                    .chars()
                    .last()
                    .map_or(false, |c| c.is_ascii_alphabetic())
                {
                    result.push(' '); // 英文行尾加空格
                }
            }
        }

        // println!("Extracted text: {}", text);
        // println!("result: {}", result);
        items.push(Item {
            content: result,
        });
        Ok(items)
    }

    fn supports(&self) -> Vec<&str> {
        vec!["pdf"]
    }
}

struct OcrReader;
impl Reader for OcrReader {
    fn read(&self, file_path: &Path) -> Result<Vec<Item>> {
        // TODO https://github.com/antimatter15/tesseract-rs/issues/39
        let tess = Tesseract::new(Some("./tessdata"), Some("eng+chi_sim")).unwrap();
        let text = tess.set_image(file_path.to_str().unwrap()).unwrap().get_text().unwrap();
        let items = text.split("\n")
            .filter(|line| !line.trim().is_empty())
            .map(|line| Item { content: line.to_string() })
            .collect();
        Ok(items)
    }

    fn supports(&self) -> Vec<&str> {
        vec!["jpg", "jpeg", "png", "tif", "tiff", "gif", "webp"]
    }
}



#[cfg(test)]
mod tests {
    use super::*;

    const TEST_DATA_DIR: &str = "../test_data/reader";

    #[test]
    fn test_composite_reader() {
        let reader = CompositeReader::new().unwrap();
        let items = reader.read(&Path::new(TEST_DATA_DIR).join("test.txt")).unwrap();
        assert_eq!(items.len(), 4);
    }

    #[test]
    fn test_composite_unknown_extension() {
        let reader = CompositeReader::new().unwrap();
        let result = reader.read(&Path::new(TEST_DATA_DIR).join("test.xyz"));
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().to_string(), "Unsupported file type");
    }

    #[test]
    fn test_txt_reader() {
        let reader = TxtReader;
        assert_eq!(reader.supports(), vec!["txt", "md", "markdown"]);
        let items = reader.read(&Path::new(TEST_DATA_DIR).join("test.txt")).unwrap();
        assert_eq!(items.len(), 4);
    }

    #[test]
    fn test_docx_reader() {
        let reader = DocxReader;
        assert_eq!(reader.supports(), vec!["docx"]);
        let items = reader
            .read(&Path::new(TEST_DATA_DIR).join("office/test.docx"))
            .unwrap();
        // println!("Items: {:?}", items);
        assert_eq!(items.len(), 10);
    }

    #[test]
    fn test_pptx_reader() {
        let reader = PptxReader;
        assert_eq!(reader.supports(), vec!["pptx"]);
        let items = reader
            .read(&Path::new(TEST_DATA_DIR).join("office/test.pptx"))
            .unwrap();
        // println!("Items: {:?}", items);
        assert_eq!(items.len(), 5);
    }

    #[test]
    fn test_pdf_reader() {
        let reader = PdfReader;
        assert_eq!(reader.supports(), vec!["pdf"]);
        let items = reader.read(&Path::new(TEST_DATA_DIR).join("test.pdf")).unwrap();
        // println!("Items: {:?}", items);
        assert_eq!(items.len(), 1);
    }

    #[test]
    fn test_xlsx_reader() {
        let reader = XlsxReader;
        assert_eq!(reader.supports(), vec!["xlsx"]);
        
        let xlsx_path = Path::new(TEST_DATA_DIR).join("office/test.xlsx");
        let items = reader.read(&xlsx_path).unwrap();
        // println!("XLSX Items: {:?}", items);
        assert_eq!(items.len(), 7);
    }


    #[test]
    fn test_ocr_reader() {
        const TEST_DATA_PIC_DIR: &str = "../test_data/reader/pic";

        let reader = OcrReader;
        assert_eq!(reader.supports(), vec!["jpg", "jpeg", "png", "tif", "tiff", "gif", "webp"]);

        let items = reader.read(&Path::new(TEST_DATA_PIC_DIR).join("test.jpg")).unwrap();
        // println!("OCR Items: {:?}", items);
        assert_eq!(items.len(), 6);
        let items = reader.read(&Path::new(TEST_DATA_PIC_DIR).join("test.jpeg")).unwrap();
        assert_eq!(items.len(), 6);

        let items = reader.read(&Path::new(TEST_DATA_PIC_DIR).join("test.png")).unwrap();
        assert_eq!(items.len(), 6);

        let items = reader.read(&Path::new(TEST_DATA_PIC_DIR).join("test.tif")).unwrap();
        assert_eq!(items.len(), 6);
        let items = reader.read(&Path::new(TEST_DATA_PIC_DIR).join("test.tiff")).unwrap();
        assert_eq!(items.len(), 6);

        let items = reader.read(&Path::new(TEST_DATA_PIC_DIR).join("test.gif")).unwrap();
        assert_eq!(items.len(), 6);

        let items = reader.read(&Path::new(TEST_DATA_PIC_DIR).join("test.webp")).unwrap();
        assert_eq!(items.len(), 6);
    }

}
