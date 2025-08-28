#[cfg(test)]
pub mod test {
    use std::env;
    use chrono::Local;
    use tempfile::Builder;
    use crate::{setup_backend, sqlite::close_pool};

    pub struct TestEnv {
        #[allow(dead_code)]
        pub temp_dir: tempfile::TempDir,
    }

    impl TestEnv {
        pub fn new() -> Self {
            Self::new_with_cleanup(true)
        }
        
        pub fn new_with_cleanup(auto_cleanup: bool) -> Self {
            let temp_dir;
            // 格式化时间，例如 "2025-08-28_08-53-25"
            let now = Local::now().format("%Y-%m-%d_%H-%M-%S").to_string();
            let dir_name = format!(".deepindex_test_{}_", now);
            if auto_cleanup {
                temp_dir = Builder::new()
                    .prefix(&dir_name)
                    .tempdir()
                    .unwrap();
            } else {
                temp_dir = Builder::new()
                    .prefix(&dir_name)
                    .disable_cleanup(true)
                    .tempdir()
                    .unwrap();
            }
            env::set_var("DEEPINDEX_TEST_DIR", temp_dir.path());

            setup_backend();

            TestEnv { temp_dir }
        }
    }

    impl Drop for TestEnv {
        fn drop(&mut self) {
            close_pool();
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

                // println!("临时目录路径: {}", temp_path.display());
                assert!(temp_path.exists(), "临时目录应该存在");
            }

            assert!(!temp_path.exists(), "临时目录应该被清理");
        }

        #[test]
        #[ignore]
        fn test_temp_dir_no_cleanup() {
            let temp_path;
            {
                let test_env = TestEnv::new_with_cleanup(false);
                temp_path = test_env.temp_dir.path().to_path_buf();

                // println!("临时目录路径: {:?}", temp_path.display());
                assert!(temp_path.exists(), "临时目录应该存在");
            }

            assert!(temp_path.exists(), "临时目录不应该被清理");
        }
    }
}
