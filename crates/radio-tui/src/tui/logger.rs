use std::fs::File;
use std::io::Write;
use std::path::Path;
use std::sync::{Mutex, OnceLock};

static LOG: OnceLock<Mutex<Option<File>>> = OnceLock::new();

pub fn init(path: &Path) {
    let file = File::create(path).ok();
    let _ = LOG.set(Mutex::new(file));
}

pub fn log(msg: &str) {
    let Some(cell) = LOG.get() else {
        return;
    };
    if let Ok(mut guard) = cell.lock() {
        if let Some(file) = guard.as_mut() {
            let _ = writeln!(file, "{msg}");
            let _ = file.flush();
        }
    }
}

#[macro_export]
macro_rules! log_warn {
    ($($arg:tt)*) => {
        $crate::tui::logger::log(&format!($($arg)*))
    };
}
