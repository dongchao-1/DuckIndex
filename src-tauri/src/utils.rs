use std::path::Path;
use anyhow::{Result, Context};

pub fn path_to_str(path: &Path) -> Result<&str> {
    path.to_str()
        .with_context(|| format!("Failed to convert path to string: {}", path.display()))
}

pub fn filename_to_str(path: &Path) -> Result<&str> {
    path.file_name()
        .with_context(|| format!("Failed to get filename from path: {}", path.display()))?
        .to_str()
        .with_context(|| format!("Failed to convert filename to string: {}", path.display()))
}

pub fn parent_to_str(path: &Path) -> Result<&str> {
    path.parent()
        .with_context(|| format!("Failed to get parent directory from path: {}", path.display()))?
        .to_str()
        .with_context(|| format!("Failed to convert parent directory to string: {}", path.display()))
}
