use std::path::PathBuf;

pub fn data_dir() -> PathBuf {
    let exe = std::env::current_exe().expect("cannot locate current exe");
    let dir = exe.parent().expect("exe has no parent").to_path_buf();
    dir.join("data")
}

pub fn ensure_data_dir() -> std::io::Result<PathBuf> {
    let d = data_dir();
    std::fs::create_dir_all(&d)?;
    Ok(d)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn data_dir_ends_with_data() {
        let p = data_dir();
        assert_eq!(p.file_name().unwrap(), "data");
    }
}
