
#[cfg(test)]
mod test {
    use tesseract;

    #[test]
    fn main() {
        // 初始化 Tesseract 引擎
        // https://github.com/tesseract-ocr/tessdata/blob/main/chi_sim.traineddata
        let mut tess = tesseract::Tesseract::new(Some("./tessdata"), Some("eng")).unwrap()
            .set_image("../test_data/reader/eng.jpg").unwrap();
        let text = tess.get_text().unwrap();

        // 打印识别结果
        println!("识别结果：\n{}", text);
    }
}