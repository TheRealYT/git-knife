// Hide the console window on Windows release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use git_knife::backup::Backup;
use git_knife::commits::{Branch, Commit, RepoInfo};
use git_knife::rewrite::{ApplyResult, CommitEdit, FieldChange};

#[tauri::command]
fn open_repo(path: String) -> Result<RepoInfo, String> {
    git_knife::commits::open_repo(&path)
}

#[tauri::command]
fn list_branches(path: String) -> Result<Vec<Branch>, String> {
    git_knife::commits::list_branches(&path)
}

#[tauri::command]
fn list_commits(path: String, branch: String, limit: u32) -> Result<Vec<Commit>, String> {
    git_knife::commits::list_commits(&path, &branch, limit)
}

#[tauri::command]
fn preview_edits(
    path: String,
    branch: String,
    edits: Vec<CommitEdit>,
) -> Result<Vec<FieldChange>, String> {
    git_knife::rewrite::preview_edits(&path, &branch, &edits)
}

#[tauri::command]
fn apply_edits(
    path: String,
    branch: String,
    edits: Vec<CommitEdit>,
    sign: bool,
    resign: bool,
) -> Result<ApplyResult, String> {
    git_knife::rewrite::apply_edits(&path, &branch, &edits, sign, resign)
}

#[tauri::command]
fn list_backups(path: String) -> Result<Vec<Backup>, String> {
    git_knife::backup::list_backups(&path)
}

#[tauri::command]
fn restore_backup(path: String, ref_name: String) -> Result<(), String> {
    git_knife::backup::restore_backup(&path, &ref_name)
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
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
