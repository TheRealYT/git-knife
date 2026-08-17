//! The rewrite engine.
//!
//! To change metadata on a commit without touching file content, we rebuild the
//! commit chain from the earliest edited commit up to the branch tip using
//! `git commit-tree`, reusing each commit's *original tree*. Because the tree is
//! reused verbatim, the resulting history has provably identical content — only
//! the metadata (message, author/committer name/email/date) differs, and every
//! descendant hash is recomputed correctly.
//!
//! MVP scope: linear history only. If a merge commit falls inside the rewrite
//! range we refuse rather than silently mangle it.

use std::collections::{HashMap, HashSet};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::commits;
use crate::git;

const US: char = '\u{1f}';
const RS: char = '\u{1e}';

/// A set of field overrides for a single commit. Any `None` field is left as-is.
#[derive(Deserialize, Clone)]
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

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FieldChange {
    pub hash: String,
    pub field: String,
    pub old_value: String,
    pub new_value: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApplyResult {
    pub new_head: String,
    pub backup_ref: String,
    pub rewritten_count: u32,
}

/// Raw commit data needed to faithfully rebuild a commit.
struct RawCommit {
    hash: String,
    tree: String,
    parents: Vec<String>,
    author_name: String,
    author_email: String,
    author_date: String,
    committer_name: String,
    committer_email: String,
    committer_date: String,
    message: String, // %B, the raw message
    signed: bool,    // had a signature before the rewrite
}

/// Read the full ancestry of `head` in topological order (a parent never
/// appears before its children), newest first, including tree + raw message.
/// Reversing the result yields parents-before-children, which the DAG rebuild
/// relies on.
fn read_chain(repo: &str, head: &str) -> Result<Vec<RawCommit>, String> {
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
fn pick(edit: Option<&Option<String>>, original: &str) -> String {
    match edit.and_then(|o| o.as_ref()) {
        Some(v) if !v.is_empty() => v.clone(),
        _ => original.to_string(),
    }
}

/// Build a single commit via `git commit-tree`, reusing `tree` and attaching
/// `parents` in order (0 = root, 1 = normal, 2+ = merge). Preserving every
/// parent is what makes merge commits rebuild correctly.
///
/// When `sign` is set, the commit is signed with the user's configured key
/// (`user.signingkey` / `gpg.format`), preserving a previously-signed commit.
#[allow(clippy::too_many_arguments)]
fn commit_tree(
    repo: &str,
    tree: &str,
    parents: &[String],
    author_name: &str,
    author_email: &str,
    author_date: &str,
    committer_name: &str,
    committer_email: &str,
    committer_date: &str,
    message: &str,
    sign: bool,
) -> Result<String, String> {
    let mut args: Vec<String> = vec!["commit-tree".to_string()];
    if sign {
        args.push("-S".to_string());
    }
    args.push(tree.to_string());
    for p in parents {
        args.push("-p".to_string());
        args.push(p.clone());
    }

    let env = [
        ("GIT_AUTHOR_NAME", author_name),
        ("GIT_AUTHOR_EMAIL", author_email),
        ("GIT_AUTHOR_DATE", author_date),
        ("GIT_COMMITTER_NAME", committer_name),
        ("GIT_COMMITTER_EMAIL", committer_email),
        ("GIT_COMMITTER_DATE", committer_date),
    ];

    let mut msg = message.to_string();
    if !msg.ends_with('\n') {
        msg.push('\n');
    }

    let argrefs: Vec<&str> = args.iter().map(String::as_str).collect();
    let out = git::run(repo, &argrefs, &env, Some(&msg))?;
    Ok(out.trim().to_string())
}

/// Compute the old→new field changes without touching the repository.
pub fn preview_edits(
    path: &str,
    branch: &str,
    edits: &[CommitEdit],
) -> Result<Vec<FieldChange>, String> {
    // `path` is already the toplevel (the frontend passes it through from
    // open_repo), and `git log` accepts a branch name directly — so neither a
    // rev-parse nor an open_repo round-trip is needed here.
    let chain = read_chain(path, branch)?;
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

/// Apply edits: rebuild the chain, save a backup ref, then move the branch.
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
    // `path` is already the toplevel (the frontend passes it through from
    // open_repo), and `git log` accepts a branch name directly — so neither a
    // rev-parse nor an open_repo round-trip is needed here.
    let repo = path;
    let chain = read_chain(repo, branch)?;
    // chain is newest-first, so its first entry is the branch tip.
    let head = chain
        .first()
        .map(|c| c.hash.clone())
        .ok_or_else(|| format!("Branch {branch} has no commits"))?;
    let edit_map: HashMap<&str, &CommitEdit> =
        edits.iter().map(|e| (e.hash.as_str(), e)).collect();

    // A commit must be rebuilt if it is edited or descends from an edited
    // commit. Processing oldest→newest (chain is topo-ordered newest-first, so
    // iterate reversed), a commit needs rebuilding when it is edited or any of
    // its parents already does. This naturally spans merges and side branches.
    let mut rewrite: HashSet<String> = HashSet::new();
    for c in chain.iter().rev() {
        let edited = edit_map.contains_key(c.hash.as_str());
        let parent_rebuilt = c.parents.iter().any(|p| rewrite.contains(p));
        if edited || parent_rebuilt {
            rewrite.insert(c.hash.clone());
        }
    }
    if rewrite.is_empty() {
        return Err("Nothing to apply — edited commits not found in this history.".to_string());
    }

    // Rebuild every commit in the set, oldest first, mapping each original
    // parent to its rewritten hash (or keeping it if that ancestor is untouched).
    let mut map: HashMap<String, String> = HashMap::new();
    for c in chain.iter().rev() {
        if !rewrite.contains(&c.hash) {
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

        let nh = commit_tree(
            repo,
            &c.tree,
            &new_parents,
            &author_name,
            &author_email,
            &author_date,
            &committer_name,
            &committer_email,
            &committer_date,
            &message,
            resign && c.signed,
        )
        .map_err(|err| {
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

    let new_head = map
        .get(&head)
        .cloned()
        .ok_or_else(|| "internal error: branch tip was not rebuilt".to_string())?;
    let rewritten_count = rewrite.len() as u32;

    // Save a backup ref pointing at the old tip before moving anything.
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let backup_ref = format!("refs/knife-backup/{branch}/{ts}");
    git::run(repo, &["update-ref", &backup_ref, &head], &[], None)?;

    // Move the branch ref with a compare-and-swap on the old tip. Editing a
    // branch by its ref never touches the working tree; when it happens to be
    // the checked-out branch, HEAD follows and the tree stays consistent
    // because the tip's tree is unchanged.
    if branch == "HEAD" {
        git::run(
            repo,
            &["update-ref", "--no-deref", "HEAD", &new_head, &head],
            &[],
            None,
        )?;
    } else {
        let refname = format!("refs/heads/{branch}");
        git::run(repo, &["update-ref", &refname, &new_head, &head], &[], None)?;
    }

    // Optional, transparent signature note. Best-effort: never fail the rewrite.
    if sign {
        let _ = add_signature(repo, &new_head, rewritten_count);
    }

    Ok(ApplyResult {
        new_head,
        backup_ref,
        rewritten_count,
    })
}
