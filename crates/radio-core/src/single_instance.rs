use std::path::PathBuf;

// the tui and the macos app are separate products sharing one data dir; each
// needs its own lock or launching either one kills the other mid-listen.
const TUI_LOCK: &str = "instance.pid";
pub const MACOS_LOCK: &str = "instance-macos.pid";

fn lock_path(name: &str) -> PathBuf {
    crate::paths::data_dir().join(name)
}

fn read_pid(path: &std::path::Path) -> Option<i32> {
    let text = std::fs::read_to_string(path).ok()?;
    text.trim().parse::<i32>().ok()
}

#[cfg(unix)]
fn is_alive(pid: i32) -> bool {
    pid > 0 && unsafe { libc::kill(pid, 0) } == 0
}

#[cfg(unix)]
fn terminate(pid: i32) {
    unsafe {
        libc::kill(pid, libc::SIGTERM);
    }
}

#[cfg(not(unix))]
fn is_alive(_pid: i32) -> bool {
    false
}

#[cfg(not(unix))]
fn terminate(_pid: i32) {}

pub fn take_over() {
    take_over_named(TUI_LOCK);
}

pub fn take_over_named(name: &str) {
    let path = lock_path(name);
    let me = std::process::id() as i32;

    if let Some(old) = read_pid(&path) {
        if old != me && is_alive(old) {
            terminate(old);
            wait_until_gone(old);
        }
    }

    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Err(e) = std::fs::write(&path, me.to_string()) {
        eprintln!("single-instance lock write failed: {e}");
    }
}

fn wait_until_gone(pid: i32) {
    for _ in 0..50 {
        if !is_alive(pid) {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn read_pid_parses_trimmed_number() {
        let dir = std::env::temp_dir().join("wr-si-test-read");
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("instance.pid");
        std::fs::write(&p, "  4321\n").unwrap();
        assert_eq!(read_pid(&p), Some(4321));
    }

    #[test]
    fn read_pid_none_for_missing_or_garbage() {
        let dir = std::env::temp_dir().join("wr-si-test-garbage");
        std::fs::create_dir_all(&dir).unwrap();
        let missing = dir.join("nope.pid");
        assert_eq!(read_pid(&missing), None);
        let p = dir.join("instance.pid");
        std::fs::write(&p, "not-a-pid").unwrap();
        assert_eq!(read_pid(&p), None);
    }

    #[test]
    fn current_process_is_alive() {
        assert!(is_alive(std::process::id() as i32));
    }

    #[test]
    fn pid_zero_is_not_alive() {
        assert!(!is_alive(0));
    }

    #[test]
    fn tui_lock_name_is_unchanged() {
        assert_eq!(TUI_LOCK, "instance.pid");
        assert_eq!(lock_path(TUI_LOCK).file_name().unwrap(), "instance.pid");
    }

    // the whole point of the parameter: two products must never share a pid file,
    // or each launch sigterms the other.
    #[test]
    fn distinct_names_give_distinct_locks() {
        assert_ne!(lock_path(TUI_LOCK), lock_path(MACOS_LOCK));
    }

    #[test]
    fn locks_share_the_data_dir() {
        assert_eq!(lock_path(TUI_LOCK).parent(), lock_path(MACOS_LOCK).parent());
    }
}
