//! Advisory repository locking.
//!
//! Prevents concurrent instances or operations from modifying the same
//! repository simultaneously. Acquires an exclusive advisory lock on
//! `.git/knife.lock`.

use std::fs::{File, OpenOptions};
use std::path::Path;
use fs2::FileExt;

/// An RAII advisory lock on a git repository's `.git/knife.lock`.
pub struct RepoLock {
    _file: File,
}

impl RepoLock {
    /// Acquire an exclusive advisory lock on the repository at `repo_path`.
    /// Returns an error if the lock cannot be acquired or if another process holds it.
    pub fn acquire(repo_path: &str) -> Result<Self, String> {
        let lock_dir = Path::new(repo_path).join(".git");
        if !lock_dir.exists() {
            return Err(format!(
                "Cannot acquire lock: .git directory not found in {repo_path}"
            ));
        }

        let lock_path = lock_dir.join("knife.lock");
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&lock_path)
            .map_err(|e| format!("Failed to open lock file at {}: {e}", lock_path.display()))?;

        // Try exclusive non-blocking lock so callers fail immediately if another process is operating.
        file.try_lock_exclusive().map_err(|e| {
            format!(
                "Repository is locked by another git-knife operation or process: {e}"
            )
        })?;

        Ok(Self {
            _file: file,
        })
    }
}

impl Drop for RepoLock {
    fn drop(&mut self) {
        #[allow(clippy::incompatible_msrv)]
        let _ = self._file.unlock();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_lock_acquire_and_release() {
        let temp_dir = temp_test_dir("test_lock_normal");
        let git_dir = temp_dir.join(".git");
        std::fs::create_dir_all(&git_dir).unwrap();

        let lock = RepoLock::acquire(temp_dir.to_str().unwrap());
        assert!(lock.is_ok());

        // Concurrent lock on same repo must fail while held
        let lock2 = RepoLock::acquire(temp_dir.to_str().unwrap());
        assert!(lock2.is_err());

        // Drop first lock
        drop(lock);

        // Now subsequent lock should succeed
        let lock3 = RepoLock::acquire(temp_dir.to_str().unwrap());
        assert!(lock3.is_ok());

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_lock_missing_git_dir() {
        let temp_dir = temp_test_dir("test_lock_missing_git");
        let res = RepoLock::acquire(temp_dir.to_str().unwrap());
        assert!(res.is_err());
    }

    fn temp_test_dir(name: &str) -> PathBuf {
        let mut p = std::env::temp_dir();
        p.push(format!("git_knife_test_lock_{}_{}", name, std::process::id()));
        let _ = std::fs::remove_dir_all(&p);
        p
    }
}
