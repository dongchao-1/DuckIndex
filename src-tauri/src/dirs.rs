use std::env;
use std::path::{Path, PathBuf};

use directories::ProjectDirs;

// 定义公司和应用名称，用于确定日志路径
const PROJECT_QUALIFIER: &str = "";
const PROJECT_ORGANIZATION: &str = "";
const PROJECT_APPLICATION: &str = "DuckIndex";

pub fn get_project_dirs() -> PathBuf {
    if let Ok(val) = env::var("DUCKINDEX_TEST_DIR") {
        Path::new(&val).join("data")
    } else {
        ProjectDirs::from(PROJECT_QUALIFIER, PROJECT_ORGANIZATION, PROJECT_APPLICATION)
            .unwrap()
            .data_dir()
            .to_path_buf()
    }
}

pub fn get_index_dir() -> PathBuf {
    let path = get_project_dirs().join("index");
    if !path.exists() {
        std::fs::create_dir_all(&path).unwrap();
    }
    path
}

pub fn get_log_dir() -> PathBuf {
    let path = get_project_dirs().join("log");
    if !path.exists() {
        std::fs::create_dir_all(&path).unwrap();
    }
    path
}

#[cfg(test)]
mod tests {

    use crate::test::test_mod::TestEnv;

    use super::*;

    #[test]
    fn test_get_index_dir() {
        let _env = TestEnv::new();
        let index_dir = get_index_dir();
        assert!(index_dir.exists());
    }

    #[test]
    fn test_get_log_dir() {
        let _env = TestEnv::new();
        let log_dir = get_log_dir();
        assert!(log_dir.exists());
    }
}
