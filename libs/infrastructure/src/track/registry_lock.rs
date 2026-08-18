use std::fs::{self, OpenOptions};
use std::path::Path;
use std::thread;
use std::time::{Duration, Instant};

#[cfg(unix)]
use std::os::unix::fs::OpenOptionsExt;
#[cfg(windows)]
use std::os::windows::fs::OpenOptionsExt;

use fs4::fs_std::FileExt as _;

use super::symlink_guard::{reject_symlinks_below, reject_symlinks_up_to_root};

const REGISTRY_LOCK_FILE: &str = ".registry.json.lock";
const REGISTRY_LOCK_TIMEOUT: Duration = Duration::from_secs(5);
const REGISTRY_LOCK_POLL_INTERVAL: Duration = Duration::from_millis(10);

pub(crate) fn acquire_registry_lock(path: &Path, trusted_root: &Path) -> Result<fs::File, String> {
    let registry_root =
        path.parent().ok_or_else(|| "rendered registry has no parent directory".to_owned())?;
    reject_symlinks_up_to_root(registry_root)
        .map_err(|error| format!("cannot inspect rendered registry lock root: {error}"))?;
    reject_symlinks_below(registry_root, trusted_root)
        .map_err(|error| format!("cannot inspect rendered registry lock root: {error}"))?;
    let lock_path = registry_root.join(REGISTRY_LOCK_FILE);
    reject_symlinks_below(&lock_path, trusted_root)
        .map_err(|error| format!("cannot inspect rendered registry lock: {error}"))?;
    let lock_file = open_registry_lock_file(&lock_path)
        .map_err(|error| format!("cannot open rendered registry lock: {error}"))?;
    let started = Instant::now();
    loop {
        match lock_file.try_lock_exclusive() {
            Ok(true) => return Ok(lock_file),
            Ok(false) if started.elapsed() >= REGISTRY_LOCK_TIMEOUT => {
                return Err(format!(
                    "timed out acquiring rendered registry lock: {}",
                    lock_path.display()
                ));
            }
            Ok(false) => thread::sleep(REGISTRY_LOCK_POLL_INTERVAL),
            Err(error) => return Err(format!("cannot acquire rendered registry lock: {error}")),
        }
    }
}

fn open_registry_lock_file(lock_path: &Path) -> std::io::Result<fs::File> {
    let mut options = OpenOptions::new();
    options.create(true).read(true).write(true).truncate(false);
    #[cfg(unix)]
    options.custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC);
    #[cfg(windows)]
    options.custom_flags(0x0020_0000);
    #[cfg(not(any(unix, windows)))]
    return Err(std::io::Error::new(
        std::io::ErrorKind::Unsupported,
        "atomic no-follow lock open is unavailable on this platform",
    ));
    options.open(lock_path)
}
