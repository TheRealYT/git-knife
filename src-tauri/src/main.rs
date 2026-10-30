// Hide the console window on Windows release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod backup;
mod commits;
mod git;
mod rewrite;

use serde::Serialize;

use backup::Backup;
use commits::{Branch, Commit, RepoInfo};
use rewrite::{ApplyResult, CommitEdit, FieldChange};

#[derive(Serialize)]
pub struct InitialRepo {
    path: String,
    branch: Option<String>,
}

/// The repository and/or branch to open at startup, from the command line:
///
/// - `git-knife <path> <branch>`
/// - `git-knife <path>`: when `<path>` is a directory
/// - `git-knife <branch>`: otherwise, in the repo of the directory git-knife
///   was started in, so `git knife <branch>` works inside a repo
/// - `git-knife`: that repo, if the directory is inside one; a launch from the
///   Start menu or Dock (usually in System32 or `/`) or any other path that is
///   not a git repository opens nothing.
///
/// Paths are made absolute against the directory git-knife was started in.
/// Arguments starting with `-` are ignored (older macOS adds `-psn_…` when
/// launched from Finder), except after `--`: `git-knife -- -dirname`.
#[tauri::command]
fn initial_repo() -> Option<InitialRepo> {
    let cwd = std::env::current_dir().ok()?;
    let mut after_dashes = false;
    let args: Vec<String> = std::env::args()
        .skip(1)
        .filter(|a| {
            if !after_dashes && a == "--" {
                after_dashes = true;
                return false;
            }
            after_dashes || !a.starts_with('-')
        })
        .collect();
    let path_of = |arg: &str| cwd.join(arg).to_string_lossy().into_owned();
    let cwd_repo = || commits::open_repo(&cwd.to_string_lossy()).ok().map(|i| i.path);
    match args.as_slice() {
        [] => cwd_repo().map(|path| InitialRepo { path, branch: None }),
        [one] if cwd.join(one).is_dir() => Some(InitialRepo { path: path_of(one), branch: None }),
        // Not a directory and not in a repo: a mistyped path, let opening it
        // report that.
        [one] => Some(match cwd_repo() {
            Some(path) => InitialRepo { path, branch: Some(one.clone()) },
            None => InitialRepo { path: path_of(one), branch: None },
        }),
        [path, branch, ..] => Some(InitialRepo {
            path: path_of(path),
            branch: Some(branch.clone()),
        }),
    }
}

#[tauri::command]
fn open_repo(path: String) -> Result<RepoInfo, String> {
    commits::open_repo(&path)
}

#[tauri::command]
fn list_branches(path: String) -> Result<Vec<Branch>, String> {
    commits::list_branches(&path)
}

#[tauri::command]
fn list_commits(path: String, branch: String, limit: u32) -> Result<Vec<Commit>, String> {
    commits::list_commits(&path, &branch, limit)
}

#[tauri::command]
fn preview_edits(
    path: String,
    branch: String,
    edits: Vec<CommitEdit>,
) -> Result<Vec<FieldChange>, String> {
    rewrite::preview_edits(&path, &branch, &edits)
}

#[tauri::command]
fn apply_edits(
    path: String,
    branch: String,
    edits: Vec<CommitEdit>,
    sign: bool,
    resign: bool,
) -> Result<ApplyResult, String> {
    rewrite::apply_edits(&path, &branch, &edits, sign, resign)
}

#[tauri::command]
fn list_backups(path: String) -> Result<Vec<Backup>, String> {
    backup::list_backups(&path)
}

#[tauri::command]
fn restore_backup(path: String, ref_name: String) -> Result<(), String> {
    backup::restore_backup(&path, &ref_name)
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            initial_repo,
            open_repo,
            list_branches,
            list_commits,
            preview_edits,
            apply_edits,
            list_backups,
            restore_backup
        ])
        .run(tauri::generate_context!())
        .expect("error while running git-knife");
}
