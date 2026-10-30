import { invoke } from "@tauri-apps/api/core";
import type {
  ApplyResult,
  Backup,
  Branch,
  Commit,
  CommitEdit,
  FieldChange,
  InitialRepo,
  RepoInfo,
} from "./types";

export const initialRepo = () => invoke<InitialRepo | null>("initial_repo");

export const openRepo =(path: string) => invoke<RepoInfo>("open_repo", { path });

export const listBranches = (path: string) =>
  invoke<Branch[]>("list_branches", { path });

export const listCommits = (path: string, branch: string, limit: number) =>
  invoke<Commit[]>("list_commits", { path, branch, limit });

export const previewEdits = (path: string, branch: string, edits: CommitEdit[]) =>
  invoke<FieldChange[]>("preview_edits", { path, branch, edits });

export const applyEdits = (
  path: string,
  branch: string,
  edits: CommitEdit[],
  sign: boolean,
  resign: boolean
) => invoke<ApplyResult>("apply_edits", { path, branch, edits, sign, resign });

export const listBackups = (path: string) =>
  invoke<Backup[]>("list_backups", { path });

export const restoreBackup = (path: string, refName: string) =>
  invoke<void>("restore_backup", { path, refName });
