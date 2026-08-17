// Hide the console window on Windows release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod backup;
mod commits;
mod git;
mod rewrite;

use backup::Backup;
use commits::{Branch, Commit, RepoInfo};
use rewrite::{ApplyResult, CommitEdit, FieldChange};

// Tauri 2 runs synchronous commands on the main thread, so every blocking
// command is wrapped in spawn_blocking to keep the UI responsive while git
// subprocesses run.

#[tauri::command]
async fn open_repo(path: String) -> Result<RepoInfo, String> {
    tauri::async_runtime::spawn_blocking(move || commits::open_repo(&path))
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn list_branches(path: String) -> Result<Vec<Branch>, String> {
    tauri::async_runtime::spawn_blocking(move || commits::list_branches(&path))
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn list_commits(path: String, branch: String, limit: u32) -> Result<Vec<Commit>, String> {
    tauri::async_runtime::spawn_blocking(move || commits::list_commits(&path, &branch, limit))
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn preview_edits(
    path: String,
    branch: String,
    edits: Vec<CommitEdit>,
) -> Result<Vec<FieldChange>, String> {
    tauri::async_runtime::spawn_blocking(move || rewrite::preview_edits(&path, &branch, &edits))
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn apply_edits(
    path: String,
    branch: String,
    edits: Vec<CommitEdit>,
    sign: bool,
    resign: bool,
) -> Result<ApplyResult, String> {
    tauri::async_runtime::spawn_blocking(move || {
        rewrite::apply_edits(&path, &branch, &edits, sign, resign)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn list_backups(path: String) -> Result<Vec<Backup>, String> {
    tauri::async_runtime::spawn_blocking(move || backup::list_backups(&path))
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn restore_backup(path: String, ref_name: String) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || backup::restore_backup(&path, &ref_name))
        .await
        .map_err(|e| e.to_string())?
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
