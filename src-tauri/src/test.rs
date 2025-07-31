#[cfg(test)]
pub mod test{
    use tempfile::tempdir;
    use crate::config::AppConfig;
    use crate::CONFIG;

    pub struct TestEnv {
        temp_dir: tempfile::TempDir,
    }

    impl TestEnv {
        pub fn new() -> Self {
            let temp_dir = tempdir().unwrap();

            let config = AppConfig {
                data_path: vec![String::from("../test_data")],
                index_path: temp_dir.path().to_string_lossy().to_string(),
            };
            CONFIG.set(config).unwrap();

            TestEnv { temp_dir }
        }
    }

    impl Drop for TestEnv {
        fn drop(&mut self) {
            // 这里会自动清理 temp_dir
            // 因为 TempDir 实现了 Drop，会自动删除临时目录
        }
    }
}
