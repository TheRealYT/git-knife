//! Reading repository metadata and the commit list.
//!
//! `git log` is asked to emit fields separated by the ASCII unit separator
//! (0x1f) and records separated by the record separator (0x1e), which cannot
//! occur in commit metadata, so parsing is unambiguous even with multi-line
//! bodies.

use std::collections::HashSet;

use serde::Serialize;

use crate::git;

const US: char = '\u{1f}'; // unit separator, between fields
const RS: char = '\u{1e}'; // record separator, between commits

pub fn find_subslice(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() || haystack.len() < needle.len() {
        return None;
    }
    (0..=haystack.len() - needle.len()).find(|&i| &haystack[i..i + needle.len()] == needle)
}

/// Parse raw bytes from `git cat-file --batch` to detect commits containing a `gpgsig` header.
pub fn parse_batch_signed(out: &[u8]) -> HashSet<String> {
    let mut signed = HashSet::new();
    let mut i = 0;
    while i < out.len() {
        // Header line: "<sha> <type> <size>" or "<sha> missing".
        let nl = match find_subslice(&out[i..], b"\n") {
            Some(p) => i + p,
            None => break,
        };
        let header = String::from_utf8_lossy(&out[i..nl]).to_string();
        i = nl + 1;
        let parts: Vec<&str> = header.split(' ').collect();
        if parts.len() < 3 {
            // "<sha> missing" — no content follows.
            continue;
        }
        let sha = parts[0].to_string();
        let size: usize = parts[2].trim().parse().unwrap_or(0);
        if i + size > out.len() {
            break;
        }
        let content = &out[i..i + size];
        i += size + 1; // skip content + trailing newline

        // The header block ends at the first blank line; a signature lives in
        // a `gpgsig` header within it.
        let hdr_end = find_subslice(content, b"\n\n").unwrap_or(content.len());
        let headers = &content[..hdr_end];
        if headers.starts_with(b"gpgsig ") || find_subslice(headers, b"\ngpgsig ").is_some() {
            signed.insert(sha);
        }
    }
    signed
}

/// Determine which of `hashes` are signed commits, by presence of a `gpgsig`
/// header in the raw object — independent of whether the signature can be
/// *verified* (SSH signatures report as unsigned via `%G?` without an
/// allowedSignersFile, so we must not rely on verification here).
///
/// Uses a single `git cat-file --batch` pass, framed by the byte sizes it
/// reports, so it is correct for arbitrary (multi-byte) commit content.
pub fn signed_set(repo: &str, hashes: &[String]) -> HashSet<String> {
    if hashes.is_empty() {
        return HashSet::new();
    }
    let stdin = format!("{}\n", hashes.join("\n"));
    match git::run_bytes(repo, &["cat-file", "--batch"], Some(&stdin)) {
        Ok(out) => parse_batch_signed(&out),
        Err(_) => HashSet::new(),
    }
}

#[derive(Serialize, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RepoInfo {
    /// Absolute path to the repository top level.
    pub path: String,
    /// Current branch name, or "HEAD" when detached.
    pub branch: String,
    /// Full HEAD commit hash.
    pub head: String,
    /// Upstream tracking ref (e.g. "origin/main"), if configured.
    pub upstream: Option<String>,
    /// Number of local commits not present on the upstream (i.e. unpushed).
    pub ahead_of_upstream: Option<u32>,
}

#[derive(Serialize, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Commit {
    pub hash: String,
    pub parents: Vec<String>,
    pub author_name: String,
    pub author_email: String,
    /// Strict ISO-8601 with offset, e.g. `2020-01-02T12:00:00+00:00`.
    pub author_date: String,
    pub committer_name: String,
    pub committer_email: String,
    pub committer_date: String,
    pub subject: String,
    pub body: String,
    pub is_merge: bool,
    /// True if the commit carries a signature (GPG or SSH), regardless of
    /// whether it can be verified here. Rewriting such a commit strips it.
    pub signed: bool,
}

/// Validate a path is inside a git repo and return branch/HEAD/upstream info.
pub fn open_repo(path: &str) -> Result<RepoInfo, String> {
    let top = git::run(path, &["rev-parse", "--show-toplevel"], &[], None)
        .map_err(|_| format!("Not a git repository: {path}"))?
        .trim()
        .to_string();

    let branch = git::run(&top, &["rev-parse", "--abbrev-ref", "HEAD"], &[], None)?
        .trim()
        .to_string();

    let head = git::run(&top, &["rev-parse", "HEAD"], &[], None)
        .map_err(|_| "Repository has no commits yet".to_string())?
        .trim()
        .to_string();

    let upstream = git::run(
        &top,
        &[
            "rev-parse",
            "--abbrev-ref",
            "--symbolic-full-name",
            "@{upstream}",
        ],
        &[],
        None,
    )
    .ok()
    .map(|s| s.trim().to_string());

    let ahead_of_upstream = upstream.as_ref().and_then(|up| {
        git::run(
            &top,
            &["rev-list", "--count", &format!("{up}..HEAD")],
            &[],
            None,
        )
        .ok()
        .and_then(|s| s.trim().parse::<u32>().ok())
    });

    Ok(RepoInfo {
        path: top,
        branch,
        head,
        upstream,
        ahead_of_upstream,
    })
}

#[derive(Serialize, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Branch {
    pub name: String,
    pub is_head: bool,
    pub head: String,
    pub upstream: Option<String>,
    pub ahead_of_upstream: Option<u32>,
}

/// List local branches with their tip, upstream, and unpushed count.
pub fn list_branches(repo: &str) -> Result<Vec<Branch>, String> {
    let fmt = format!(
        "--format=%(refname:short){US}%(objectname){US}%(upstream:short){US}%(HEAD)"
    );
    let out = git::run(repo, &["for-each-ref", &fmt, "refs/heads"], &[], None)?;

    let mut branches = Vec::new();
    for line in out.lines() {
        if line.trim().is_empty() {
            continue;
        }
        let f: Vec<&str> = line.split(US).collect();
        if f.len() < 4 {
            continue;
        }
        let name = f[0].to_string();
        let upstream = if f[2].is_empty() {
            None
        } else {
            Some(f[2].to_string())
        };
        let ahead_of_upstream = upstream.as_ref().and_then(|up| {
            git::run(
                repo,
                &["rev-list", "--count", &format!("{up}..{name}")],
                &[],
                None,
            )
            .ok()
            .and_then(|s| s.trim().parse::<u32>().ok())
        });
        branches.push(Branch {
            name,
            is_head: f[3].trim() == "*",
            head: f[1].to_string(),
            upstream,
            ahead_of_upstream,
        });
    }
    Ok(branches)
}

/// List up to `limit` commits reachable from `branch`, newest first.
pub fn list_commits(repo: &str, branch: &str, limit: u32) -> Result<Vec<Commit>, String> {
    let limit = limit.max(1);
    let fmt = format!(
        "--format=%H{US}%P{US}%an{US}%ae{US}%aI{US}%cn{US}%ce{US}%cI{US}%s{US}%b{RS}"
    );
    let out = git::run(
        repo,
        &["log", &format!("-{limit}"), &fmt, branch],
        &[],
        None,
    )?;

    let mut commits = Vec::new();
    for record in out.split(RS) {
        let record = record.trim_start_matches('\n');
        if record.trim().is_empty() {
            continue;
        }
        let f: Vec<&str> = record.split(US).collect();
        if f.len() < 10 {
            continue;
        }
        let parents: Vec<String> = f[1].split_whitespace().map(str::to_string).collect();
        let is_merge = parents.len() > 1;
        commits.push(Commit {
            hash: f[0].to_string(),
            parents,
            author_name: f[2].to_string(),
            author_email: f[3].to_string(),
            author_date: f[4].to_string(),
            committer_name: f[5].to_string(),
            committer_email: f[6].to_string(),
            committer_date: f[7].to_string(),
            signed: false, // filled in below
            subject: f[8].to_string(),
            body: f[9].to_string(),
            is_merge,
        });
    }

    // Mark signed commits via raw-header scan (verification-independent).
    let hashes: Vec<String> = commits.iter().map(|c| c.hash.clone()).collect();
    let signed = signed_set(repo, &hashes);
    for c in commits.iter_mut() {
        c.signed = signed.contains(&c.hash);
    }

    Ok(commits)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_find_subslice_basic() {
        let h = b"hello world\n\ngoodbye";
        assert_eq!(find_subslice(h, b"\n\n"), Some(11));
        assert_eq!(find_subslice(h, b"hello"), Some(0));
        assert_eq!(find_subslice(h, b"missing"), None);
        assert_eq!(find_subslice(h, b""), None);
    }

    #[test]
    fn test_parse_batch_signed_signed_and_unsigned() {
        let raw_signed = b"tree 4b825dc642cb6eb9a060e54bf8d69288fbee4904\n\
gpgsig -----BEGIN SSH SIGNATURE-----\n\
 U1NIU0lHAAAA...==\n\
 -----END SSH SIGNATURE-----\n\
author Test <test@test.com> 1700000000 +0000\n\
committer Test <test@test.com> 1700000000 +0000\n\
\n\
Signed commit message\n";

        let raw_unsigned = b"tree 4b825dc642cb6eb9a060e54bf8d69288fbee4904\n\
author Test <test@test.com> 1700000000 +0000\n\
committer Test <test@test.com> 1700000000 +0000\n\
\n\
Unsigned commit message\n";

        let mut output = Vec::new();
        // Item 1: signed
        output.extend_from_slice(format!("sha1commit commit {}\n", raw_signed.len()).as_bytes());
        output.extend_from_slice(raw_signed);
        output.push(b'\n');

        // Item 2: unsigned
        output.extend_from_slice(format!("sha2commit commit {}\n", raw_unsigned.len()).as_bytes());
        output.extend_from_slice(raw_unsigned);
        output.push(b'\n');

        // Item 3: missing
        output.extend_from_slice(b"sha3missing missing\n");

        let signed = parse_batch_signed(&output);
        assert!(signed.contains("sha1commit"));
        assert!(!signed.contains("sha2commit"));
        assert!(!signed.contains("sha3missing"));
    }

    #[test]
    fn test_parse_batch_signed_empty() {
        let signed = parse_batch_signed(b"");
        assert!(signed.is_empty());
    }
}
