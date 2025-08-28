#[cfg(test)]
pub mod test {
    use std::env;
    use tempfile::Builder;
    use crate::setup_backend;

    pub struct TestEnv {
        #[allow(dead_code)]
        pub temp_dir: tempfile::TempDir,
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

    #[cfg(test)]
    mod temp_dir_tests {
        use super::*;

        #[test]
        fn test_temp_dir_cleanup() {
            let temp_path;
            {
                let test_env = TestEnv::new();
                temp_path = test_env.temp_dir.path().to_path_buf();

                // 在作用域内，临时目录应该存在
                println!("临时目录路径: {:?}", temp_path);
                assert!(temp_path.exists(), "临时目录应该存在");
            } // test_env 在这里被 drop

            // 在作用域外，临时目录应该被清理
            assert!(!temp_path.exists(), "临时目录应该被清理");
            println!("临时目录已被清理: {:?}", temp_path);
        }
    }
}
