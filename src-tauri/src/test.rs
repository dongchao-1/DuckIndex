#[cfg(test)]
pub mod test_mod {
    use crate::{setup_backend, sqlite::close_pool};
    use chrono::Local;
    use std::env;
    use tempfile::Builder;

    pub struct TestEnv {
        #[allow(dead_code)]
        pub temp_dir: tempfile::TempDir,
    }

    impl TestEnv {
        pub fn new() -> Self {
            Self::new_with_cleanup(true)
        }

        pub fn new_with_cleanup(auto_cleanup: bool) -> Self {
            let now = Local::now().format("%Y-%m-%d_%H-%M-%S").to_string();
            let dir_name = format!(".deepindex_test_{now}_");
            let temp_dir = if auto_cleanup {
                Builder::new().prefix(&dir_name).tempdir().unwrap()
            } else {
                Builder::new()
                    .prefix(&dir_name)
                    .disable_cleanup(true)
                    .tempdir()
                    .unwrap()
            };
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
