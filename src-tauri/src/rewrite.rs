//! The rewrite engine.
//!
//! To change metadata on a commit without touching file content, we rebuild the
//! commit chain from the earliest edited commit up to the branch tip using
//! `git commit-tree`, reusing each commit's *original tree*. Because the tree is
//! reused verbatim, the resulting history has provably identical content — only
//! the metadata (message, author/committer name/email/date) differs, and every
//! descendant hash is recomputed correctly.
//!
//! Non-linear DAG history and merge commits are fully supported: git-knife
//! reconstructs the full commit graph, preserving all parents for each merge commit.

use std::collections::{HashMap, HashSet};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::commits;
use crate::git;
use crate::lock::RepoLock;

const US: char = '\u{1f}';
const RS: char = '\u{1e}';

/// A set of field overrides for a single commit. Any `None` field is left as-is.
#[derive(Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct CommitEdit {
    pub hash: String,
    #[serde(default)]
    pub author_name: Option<String>,
    #[serde(default)]
    pub author_email: Option<String>,
    #[serde(default)]
    pub author_date: Option<String>,
    #[serde(default)]
    pub committer_name: Option<String>,
    #[serde(default)]
    pub committer_email: Option<String>,
    #[serde(default)]
    pub committer_date: Option<String>,
    /// Full commit message (subject + body). Replaces the entire message.
    #[serde(default)]
    pub message: Option<String>,
}

#[derive(Serialize, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct FieldChange {
    pub hash: String,
    pub field: String,
    pub old_value: String,
    pub new_value: String,
}

#[derive(Serialize, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ApplyResult {
    pub new_head: String,
    pub backup_ref: String,
    pub rewritten_count: u32,
}

/// Raw commit data needed to faithfully rebuild a commit.
#[derive(Debug, Clone)]
pub struct RawCommit {
    pub hash: String,
    pub tree: String,
    pub parents: Vec<String>,
    pub author_name: String,
    pub author_email: String,
    pub author_date: String,
    pub committer_name: String,
    pub committer_email: String,
    pub committer_date: String,
    pub message: String, // %B, the raw message
    pub signed: bool,    // had a signature before the rewrite
}

/// Read the full ancestry of `head` in topological order (a parent never
/// appears before its children), newest first, including tree + raw message.
/// Reversing the result yields parents-before-children, which the DAG rebuild
/// relies on.
pub fn read_chain(repo: &str, head: &str) -> Result<Vec<RawCommit>, String> {
    let fmt = format!(
        "--format=%H{US}%T{US}%P{US}%an{US}%ae{US}%aI{US}%cn{US}%ce{US}%cI{US}%B{RS}"
    );
    let out = git::run(repo, &["log", "--topo-order", &fmt, head], &[], None)?;

    let mut chain = Vec::new();
    for record in out.split(RS) {
        let record = record.trim_start_matches('\n');
        if record.trim().is_empty() {
            continue;
        }
        let f: Vec<&str> = record.split(US).collect();
        if f.len() < 10 {
            continue;
        }
        let parents: Vec<String> = f[2].split_whitespace().map(str::to_string).collect();
        chain.push(RawCommit {
            hash: f[0].to_string(),
            tree: f[1].to_string(),
            parents,
            author_name: f[3].to_string(),
            author_email: f[4].to_string(),
            author_date: f[5].to_string(),
            committer_name: f[6].to_string(),
            committer_email: f[7].to_string(),
            committer_date: f[8].to_string(),
            signed: false, // filled in below
            message: f[9].to_string(),
        });
    }

    // Verification-independent signature detection.
    let hashes: Vec<String> = chain.iter().map(|c| c.hash.clone()).collect();
    let signed = commits::signed_set(repo, &hashes);
    for c in chain.iter_mut() {
        c.signed = signed.contains(&c.hash);
    }

    Ok(chain)
}

/// Resolve the effective value: a non-empty edit override, else the original.
pub fn pick(edit: Option<&Option<String>>, original: &str) -> String {
    match edit.and_then(|o| o.as_ref()) {
        Some(v) if !v.is_empty() => v.clone(),
        _ => original.to_string(),
    }
}

/// Configuration parameters required to build a commit with `git commit-tree`.
#[derive(Debug, Clone)]
pub struct CommitTreeConfig<'a> {
    pub repo: &'a str,
    pub tree: &'a str,
    pub parents: &'a [String],
    pub author_name: &'a str,
    pub author_email: &'a str,
    pub author_date: &'a str,
    pub committer_name: &'a str,
    pub committer_email: &'a str,
    pub committer_date: &'a str,
    pub message: &'a str,
    pub sign: bool,
}

/// Build a single commit via `git commit-tree`, reusing `tree` and attaching
/// `parents` in order (0 = root, 1 = normal, 2+ = merge). Preserving every
/// parent is what makes merge commits rebuild correctly.
///
/// When `sign` is set, the commit is signed with the user's configured key
/// (`user.signingkey` / `gpg.format`), preserving a previously-signed commit.
pub fn commit_tree(config: &CommitTreeConfig) -> Result<String, String> {
    let mut args: Vec<String> = vec!["commit-tree".to_string()];
    if config.sign {
        args.push("-S".to_string());
    }
    args.push(config.tree.to_string());
    for p in config.parents {
        args.push("-p".to_string());
        args.push(p.clone());
    }

    let env = [
        ("GIT_AUTHOR_NAME", config.author_name),
        ("GIT_AUTHOR_EMAIL", config.author_email),
        ("GIT_AUTHOR_DATE", config.author_date),
        ("GIT_COMMITTER_NAME", config.committer_name),
        ("GIT_COMMITTER_EMAIL", config.committer_email),
        ("GIT_COMMITTER_DATE", config.committer_date),
    ];

    let mut msg = config.message.to_string();
    if !msg.ends_with('\n') {
        msg.push('\n');
    }

    let argrefs: Vec<&str> = args.iter().map(String::as_str).collect();
    let out = git::run(config.repo, &argrefs, &env, Some(&msg))?;
    Ok(out.trim().to_string())
}

/// Compute the old→new field changes without touching the repository.
pub fn preview_edits(
    path: &str,
    branch: &str,
    edits: &[CommitEdit],
) -> Result<Vec<FieldChange>, String> {
    let info = commits::open_repo(path)?;
    let tip = git::run(&info.path, &["rev-parse", branch], &[], None)?
        .trim()
        .to_string();
    let chain = read_chain(&info.path, &tip)?;
    let by_hash: HashMap<&str, &RawCommit> = chain.iter().map(|c| (c.hash.as_str(), c)).collect();

    let mut changes = Vec::new();
    for e in edits {
        let Some(c) = by_hash.get(e.hash.as_str()) else {
            continue;
        };
        let scalar_fields: [(&str, &str, &Option<String>); 6] = [
            ("author name", c.author_name.as_str(), &e.author_name),
            ("author email", c.author_email.as_str(), &e.author_email),
            ("author date", c.author_date.as_str(), &e.author_date),
            ("committer name", c.committer_name.as_str(), &e.committer_name),
            ("committer email", c.committer_email.as_str(), &e.committer_email),
            ("committer date", c.committer_date.as_str(), &e.committer_date),
        ];
        for (field, old, new) in scalar_fields {
            if let Some(v) = new {
                if !v.is_empty() && v != old {
                    changes.push(FieldChange {
                        hash: c.hash.clone(),
                        field: field.to_string(),
                        old_value: old.to_string(),
                        new_value: v.clone(),
                    });
                }
            }
        }
        // Message compares against the trimmed raw body to avoid trailing-newline noise.
        if let Some(m) = &e.message {
            if m.trim_end() != c.message.trim_end() {
                changes.push(FieldChange {
                    hash: c.hash.clone(),
                    field: "message".to_string(),
                    old_value: c.message.trim_end().to_string(),
                    new_value: m.trim_end().to_string(),
                });
            }
        }
    }
    Ok(changes)
}

/// Notes ref used for the (optional, transparent) git-knife signature.
const NOTES_REF: &str = "refs/notes/git-knife";

/// Attach a signature note to the rewritten tip. Uses a dedicated notes ref so
/// it never touches the user's default notes. Best-effort — callers ignore the
/// result so a notes failure never fails the rewrite itself.
fn add_signature(repo: &str, commit: &str, count: u32) -> Result<(), String> {
    let msg = format!(
        "Stabbed with git-knife 🔪 ({count} commit{} re-authored).\n\
         https://github.com/TheRealYT/git-knife",
        if count == 1 { "" } else { "s" }
    );
    git::run(
        repo,
        &[
            "notes",
            &format!("--ref={NOTES_REF}"),
            "add",
            "-f",
            "-m",
            &msg,
            commit,
        ],
        &[],
        None,
    )?;
    Ok(())
}

/// Computes the set of commit hashes that must be rebuilt.
///
/// A commit must be rebuilt if it is edited or descends from an edited commit.
/// Processing oldest→newest (chain is topo-ordered newest-first, so iterate reversed),
/// a commit needs rebuilding when it is edited or any of its parents already does.
/// This naturally spans merges and side branches.
pub fn compute_rewrite_set(
    chain: &[RawCommit],
    edit_map: &HashMap<&str, &CommitEdit>,
) -> HashSet<String> {
    let mut rewrite: HashSet<String> = HashSet::new();
    for c in chain.iter().rev() {
        let edited = edit_map.contains_key(c.hash.as_str());
        let parent_rebuilt = c.parents.iter().any(|p| rewrite.contains(p));
        if edited || parent_rebuilt {
            rewrite.insert(c.hash.clone());
        }
    }
    rewrite
}

/// Rebuild every commit in the rewrite set, oldest first, mapping each original
/// parent to its rewritten hash (or keeping it if that ancestor is untouched).
///
/// Returns a map of original commit hashes to their newly created commit hashes.
pub fn rebuild_chain(
    repo: &str,
    chain: &[RawCommit],
    rewrite_set: &HashSet<String>,
    edit_map: &HashMap<&str, &CommitEdit>,
    resign: bool,
) -> Result<HashMap<String, String>, String> {
    let mut map: HashMap<String, String> = HashMap::new();
    for c in chain.iter().rev() {
        if !rewrite_set.contains(&c.hash) {
            continue;
        }
        let e = edit_map.get(c.hash.as_str()).copied();

        let author_name = pick(e.map(|e| &e.author_name), &c.author_name);
        let author_email = pick(e.map(|e| &e.author_email), &c.author_email);
        let author_date = pick(e.map(|e| &e.author_date), &c.author_date);
        let committer_name = pick(e.map(|e| &e.committer_name), &c.committer_name);
        let committer_email = pick(e.map(|e| &e.committer_email), &c.committer_email);
        let committer_date = pick(e.map(|e| &e.committer_date), &c.committer_date);
        let message = match e.and_then(|e| e.message.clone()) {
            Some(m) if !m.trim().is_empty() => m,
            _ => c.message.clone(),
        };

        let new_parents: Vec<String> = c
            .parents
            .iter()
            .map(|p| map.get(p).cloned().unwrap_or_else(|| p.clone()))
            .collect();

        let config = CommitTreeConfig {
            repo,
            tree: &c.tree,
            parents: &new_parents,
            author_name: &author_name,
            author_email: &author_email,
            author_date: &author_date,
            committer_name: &committer_name,
            committer_email: &committer_email,
            committer_date: &committer_date,
            message: &message,
            sign: resign && c.signed,
        };

        let nh = commit_tree(&config).map_err(|err| {
            if resign && c.signed {
                format!(
                    "Failed to re-sign commit {} — is a signing key configured \
                     (user.signingkey / gpg.format)? Underlying error: {err}",
                    &c.hash[..c.hash.len().min(8)]
                )
            } else {
                err
            }
        })?;
        map.insert(c.hash.clone(), nh);
    }
    Ok(map)
}

/// Atomically save a backup ref and update the branch ref to point at the new tip.
pub fn update_refs(
    repo: &str,
    branch: &str,
    old_head: &str,
    new_head: &str,
    rewritten_count: u32,
    sign: bool,
) -> Result<ApplyResult, String> {
    // Save a backup ref pointing at the old tip before moving anything.
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let backup_ref = format!("refs/knife-backup/{branch}/{ts}");
    git::run(repo, &["update-ref", &backup_ref, old_head], &[], None)?;

    // Move the branch ref with a compare-and-swap on the old tip. Editing a
    // branch by its ref never touches the working tree; when it happens to be
    // the checked-out branch, HEAD follows and the tree stays consistent
    // because the tip's tree is unchanged.
    if branch == "HEAD" {
        git::run(
            repo,
            &["update-ref", "--no-deref", "HEAD", new_head, old_head],
            &[],
            None,
        )?;
    } else {
        let refname = format!("refs/heads/{branch}");
        git::run(repo, &["update-ref", &refname, new_head, old_head], &[], None)?;
    }

    // Optional, transparent signature note. Best-effort: never fail the rewrite.
    if sign {
        let _ = add_signature(repo, new_head, rewritten_count);
    }

    Ok(ApplyResult {
        new_head: new_head.to_string(),
        backup_ref,
        rewritten_count,
    })
}

/// Apply edits: acquire advisory repo lock, rebuild the chain, save a backup ref,
/// and update the branch.
///
/// * `sign`   — attach a transparent git-knife signature note to the new tip.
/// * `resign` — re-sign rebuilt commits that were signed before, with the
///   user's configured key (otherwise a rewrite silently drops signatures).
pub fn apply_edits(
    path: &str,
    branch: &str,
    edits: &[CommitEdit],
    sign: bool,
    resign: bool,
) -> Result<ApplyResult, String> {
    let info = commits::open_repo(path)?;
    let repo = info.path.as_str();

    // Acquire advisory file lock to prevent race conditions with concurrent operations.
    let _lock = RepoLock::acquire(repo)?;

    // Resolve the tip of the *selected* branch (not necessarily HEAD).
    let head = git::run(repo, &["rev-parse", branch], &[], None)?
        .trim()
        .to_string();

    let chain = read_chain(repo, &head)?;
    let edit_map: HashMap<&str, &CommitEdit> =
        edits.iter().map(|e| (e.hash.as_str(), e)).collect();

    let rewrite = compute_rewrite_set(&chain, &edit_map);
    if rewrite.is_empty() {
        return Err("Nothing to apply — edited commits not found in this history.".to_string());
    }

    let map = rebuild_chain(repo, &chain, &rewrite, &edit_map, resign)?;

    let new_head = map
        .get(&head)
        .cloned()
        .ok_or_else(|| "internal error: branch tip was not rebuilt".to_string())?;
    let rewritten_count = rewrite.len() as u32;

    update_refs(repo, branch, &head, &new_head, rewritten_count, sign)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pick_returns_edit_when_nonempty() {
        let edit = Some("alice@example.com".to_string());
        let original = "bob@example.com";
        assert_eq!(pick(Some(&edit), original), "alice@example.com");
    }

    #[test]
    fn test_pick_returns_original_when_empty_or_none() {
        let empty = Some("".to_string());
        let none: Option<String> = None;
        let original = "bob@example.com";
        assert_eq!(pick(Some(&empty), original), original);
        assert_eq!(pick(Some(&none), original), original);
        assert_eq!(pick(None, original), original);
    }

    fn make_test_commit(hash: &str, parents: Vec<&str>) -> RawCommit {
        RawCommit {
            hash: hash.to_string(),
            tree: "4b825dc642cb6eb9a060e54bf8d69288fbee4904".to_string(),
            parents: parents.into_iter().map(str::to_string).collect(),
            author_name: "Author".to_string(),
            author_email: "author@test.com".to_string(),
            author_date: "2024-01-01T12:00:00+00:00".to_string(),
            committer_name: "Committer".to_string(),
            committer_email: "committer@test.com".to_string(),
            committer_date: "2024-01-01T12:00:00+00:00".to_string(),
            message: "Commit message\n".to_string(),
            signed: false,
        }
    }

    #[test]
    fn test_compute_rewrite_set_linear_propagation() {
        // Linear chain: C3 -> C2 -> C1
        let c1 = make_test_commit("c1", vec![]);
        let c2 = make_test_commit("c2", vec!["c1"]);
        let c3 = make_test_commit("c3", vec!["c2"]);
        let chain = vec![c3, c2, c1]; // newest first

        let edit_c2 = CommitEdit {
            hash: "c2".to_string(),
            author_name: Some("New Name".to_string()),
            ..Default::default()
        };
        let mut edits = HashMap::new();
        edits.insert("c2", &edit_c2);

        let rewrite = compute_rewrite_set(&chain, &edits);
        assert_eq!(rewrite.len(), 2);
        assert!(rewrite.contains("c2"));
        assert!(rewrite.contains("c3"));
        assert!(!rewrite.contains("c1"));
    }

    #[test]
    fn test_compute_rewrite_set_merge_commit_propagation() {
        // DAG with merge:
        // C4 (merge c3, c2b)
        // c3 -> c2a -> c1
        // c2b -> c1
        let c1 = make_test_commit("c1", vec![]);
        let c2a = make_test_commit("c2a", vec!["c1"]);
        let c2b = make_test_commit("c2b", vec!["c1"]);
        let c3 = make_test_commit("c3", vec!["c2a"]);
        let c4 = make_test_commit("c4", vec!["c3", "c2b"]);
        let chain = vec![c4, c3, c2b, c2a, c1]; // topo-ordered newest first

        // Edit c2b (side branch)
        let edit_c2b = CommitEdit {
            hash: "c2b".to_string(),
            message: Some("Updated branch msg".to_string()),
            ..Default::default()
        };
        let mut edits = HashMap::new();
        edits.insert("c2b", &edit_c2b);

        let rewrite = compute_rewrite_set(&chain, &edits);
        assert!(rewrite.contains("c2b"));
        assert!(rewrite.contains("c4")); // merge commit must be rebuilt
        assert!(!rewrite.contains("c1"));
        assert!(!rewrite.contains("c2a"));
        assert!(!rewrite.contains("c3"));
    }

    #[test]
    fn test_compute_rewrite_set_no_matches() {
        let c1 = make_test_commit("c1", vec![]);
        let chain = vec![c1];
        let edit_other = CommitEdit {
            hash: "c_unknown".to_string(),
            ..Default::default()
        };
        let mut edits = HashMap::new();
        edits.insert("c_unknown", &edit_other);

        let rewrite = compute_rewrite_set(&chain, &edits);
        assert!(rewrite.is_empty());
    }
}
