//! Backup refs created before each rewrite, and restoring from them.
//!
//! Backups live under `refs/knife-backup/<branch>/<epoch-seconds>` and point at
//! the branch tip as it was immediately before an apply. Restoring is a plain
//! `git reset --hard` to that commit.

use serde::Serialize;

use crate::commits;
use crate::git;
use crate::lock::RepoLock;

const US: char = '\u{1f}';

#[derive(Serialize, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Backup {
    pub ref_name: String,
    /// Commit hash the backup points at.
    pub target: String,
    /// Unix epoch seconds (as a string) parsed from the ref name.
    pub created_at: String,
    pub branch: String,
    /// Subject of the target commit, for display.
    pub subject: String,
}

/// Parse a raw for-each-ref output line into a Backup struct.
pub fn parse_backup_line(line: &str) -> Option<Backup> {
    if line.trim().is_empty() {
        return None;
    }
    let f: Vec<&str> = line.split(US).collect();
    if f.len() < 3 {
        return None;
    }
    let ref_name = f[0].to_string();
    let rest = ref_name
        .strip_prefix("refs/knife-backup/")
        .unwrap_or(&ref_name);
    // Branch may contain slashes; the final segment is the timestamp.
    let (branch, created_at) = match rest.rsplit_once('/') {
        Some((b, t)) => (b.to_string(), t.to_string()),
        None => (String::new(), rest.to_string()),
    };
    Some(Backup {
        ref_name,
        target: f[1].to_string(),
        created_at,
        branch,
        subject: f[2].to_string(),
    })
}

/// List all knife backup refs, newest first.
pub fn list_backups(path: &str) -> Result<Vec<Backup>, String> {
    let info = commits::open_repo(path)?;
    let repo = info.path.as_str();

    let out = git::run(
        repo,
        &[
            "for-each-ref",
            &format!("--format=%(refname){US}%(objectname){US}%(subject)"),
            "refs/knife-backup",
        ],
        &[],
        None,
    )?;

    let mut backups = Vec::new();
    for line in out.lines() {
        if let Some(b) = parse_backup_line(line) {
            backups.push(b);
        }
    }

    // Newest first by epoch timestamp.
    backups.sort_by(|a, b| {
        let ai = a.created_at.parse::<u64>().unwrap_or(0);
        let bi = b.created_at.parse::<u64>().unwrap_or(0);
        bi.cmp(&ai)
    });

    Ok(backups)
}

/// Restore a backup. The branch is parsed from the backup ref name. If it is
/// the currently checked-out branch we `git reset --hard` (updating the working
/// tree); otherwise we just move that branch's ref, leaving the checkout alone.
pub fn restore_backup(path: &str, ref_name: &str) -> Result<(), String> {
    if !ref_name.starts_with("refs/knife-backup/") {
        return Err("Refusing to restore a ref outside refs/knife-backup/.".to_string());
    }
    let info = commits::open_repo(path)?;
    let repo = info.path.as_str();

    // Acquire advisory repository lock before restoring
    let _lock = RepoLock::acquire(repo)?;

    let target = git::run(repo, &["rev-parse", ref_name], &[], None)?
        .trim()
        .to_string();

    // refs/knife-backup/<branch>/<ts> — branch may contain slashes.
    let rest = ref_name.strip_prefix("refs/knife-backup/").unwrap_or("");
    let branch = rest.rsplit_once('/').map(|(b, _)| b).unwrap_or("");

    if branch.is_empty() {
        return Err("Could not determine the branch for this backup.".to_string());
    }

    if branch == info.branch {
        // Checked-out branch: move ref + working tree together.
        git::run(repo, &["reset", "--hard", &target], &[], None)?;
    } else {
        // Other branch: move its ref only, don't disturb the checkout.
        let refname = format!("refs/heads/{branch}");
        git::run(repo, &["update-ref", &refname, &target], &[], None)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_backup_line_simple_branch() {
        let line = format!("refs/knife-backup/main/1700000000{US}a1b2c3d4{US}Initial commit");
        let b = parse_backup_line(&line).expect("should parse");
        assert_eq!(b.ref_name, "refs/knife-backup/main/1700000000");
        assert_eq!(b.target, "a1b2c3d4");
        assert_eq!(b.created_at, "1700000000");
        assert_eq!(b.branch, "main");
        assert_eq!(b.subject, "Initial commit");
    }

    #[test]
    fn test_parse_backup_line_nested_slash_branch() {
        let line = format!("refs/knife-backup/feat/login/oauth/1700000050{US}e5f6g7h8{US}Add OAuth");
        let b = parse_backup_line(&line).expect("should parse");
        assert_eq!(b.branch, "feat/login/oauth");
        assert_eq!(b.created_at, "1700000050");
        assert_eq!(b.target, "e5f6g7h8");
        assert_eq!(b.subject, "Add OAuth");
    }

    #[test]
    fn test_parse_backup_line_empty() {
        assert!(parse_backup_line("").is_none());
        assert!(parse_backup_line("   \n").is_none());
    }
}
