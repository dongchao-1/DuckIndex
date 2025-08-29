use tesseract::{Tesseract, lang};
use image::io::Reader as ImageReader;
use std::path::Path;

#[cfg(test)]
mod tests
{
    #[test]
    fn test_ocr() {
        // 假设你有一个图片文件
        let image_path = Path::new("../test_data/eng.png");

        // Tesseract 库需要系统安装，你可以在这里指定语言包的路径，
        // 如果不指定，它会去默认位置查找
        let mut tess = Tesseract::new(None, Some(lang::eng)).unwrap();

        // 使用 image crate 打开并解码图片
        let image = ImageReader::open(image_path)
            .expect("Failed to open image")
            .decode()
            .expect("Failed to decode image");

        // 将图像设置为 OCR 引擎的输入
        tess.set_image(&image).unwrap();

        // 执行 OCR 并获取结果
        match tess.recognize() {
            Ok(text) => {
                println!("识别结果：\n{}", text);
            }
            Err(e) => {
                eprintln!("OCR 失败：{}", e);
            }
        }
    }
}