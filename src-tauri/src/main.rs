// Hide the console window on Windows release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod backup;
mod commits;
mod git;
mod rewrite;

use backup::Backup;
use commits::{Branch, Commit, RepoInfo};
use rewrite::{ApplyResult, CommitEdit, FieldChange};

/// The repository given on the command line (`git-knife <path>`), made
/// absolute against the directory git-knife was started in, so `git-knife .`
/// opens the current repo. Flags (e.g. macOS's `-psn_…`) are skipped.
#[tauri::command]
fn initial_path() -> Option<String> {
    let arg = std::env::args().skip(1).find(|a| !a.starts_with('-'))?;
    let cwd = std::env::current_dir().ok()?;
    Some(cwd.join(arg).to_string_lossy().into_owned())
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
            initial_path,
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
