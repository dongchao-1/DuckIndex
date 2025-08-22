#[cfg(test)]
pub mod test {
    use std::env;
    use tempfile::Builder;

    use crate::setup_backend;

    pub struct TestEnv {
        #[allow(dead_code)]
        temp_dir: tempfile::TempDir,
    }

    impl TestEnv {
        pub fn new() -> Self {
            let temp_dir = Builder::new()
                .prefix(".deepindex_") // 设置前缀
                .tempdir()
                .unwrap(); // 创建临时目录
            env::set_var("DEEPINDEX_TEST_DIR", temp_dir.path());

            setup_backend();

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
