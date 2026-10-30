import { useEffect, useMemo, useRef, useState } from "react";
import { open as openDialog, ask } from "@tauri-apps/plugin-dialog";
import {
  applyEdits,
  initialRepo,
  listBackups,
  listBranches,
  listCommits,
  openRepo,
  previewEdits,
  restoreBackup,
} from "./api";
import type {
  Backup,
  Branch,
  Commit,
  CommitEdit,
  FieldChange,
  RepoInfo,
} from "./types";
import CommitTable, { type EditableField, fullMessage } from "./components/CommitTable";
import ApplyBar, { ConfirmDialog } from "./components/ApplyBar";
import BackupPanel from "./components/BackupPanel";
import BulkPanel from "./components/BulkPanel";
import {
  computeBulk,
  emptyBulkSpec,
  type BulkField,
  type BulkSpec,
} from "./bulk";

const LIMIT = 200;

export default function App() {
  const [pathInput, setPathInput] = useState("");
  const [repo, setRepo] = useState<RepoInfo | null>(null);
  const [branches, setBranches] = useState<Branch[]>([]);
  const [branch, setBranch] = useState<string>("");
  const [commits, setCommits] = useState<Commit[]>([]);
  const [backups, setBackups] = useState<Backup[]>([]);
  const [edits, setEdits] = useState<Record<string, CommitEdit>>({});
  const [expanded, setExpanded] = useState<string | null>(null);
  const [confirm, setConfirm] = useState<FieldChange[] | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const [bulkOpen, setBulkOpen] = useState(false);
  const [bulk, setBulk] = useState<BulkSpec>(emptyBulkSpec);
  // Transparent signature note on rewrites — on by default, persisted, opt-out.
  const [sign, setSign] = useState<boolean>(
    () => localStorage.getItem("knife.sign") !== "0"
  );

  function toggleSign(next: boolean) {
    setSign(next);
    localStorage.setItem("knife.sign", next ? "1" : "0");
  }

  // Re-sign rebuilt commits that were signed (opt-in, persisted).
  const [resign, setResign] = useState<boolean>(
    () => localStorage.getItem("knife.resign") === "1"
  );

  function toggleResign(next: boolean) {
    setResign(next);
    localStorage.setItem("knife.resign", next ? "1" : "0");
  }

  const dirtyCount = Object.keys(edits).length;

  const commitByHash = useMemo(() => {
    const m: Record<string, Commit> = {};
    commits.forEach((c) => (m[c.hash] = c));
    return m;
  }, [commits]);

  const selectedBranch = branches.find((b) => b.name === branch) ?? null;

  // Live preview of the bulk find & replace against current commits + edits.
  const bulkResult = useMemo(
    () => computeBulk(commits, edits, bulk),
    [commits, edits, bulk]
  );

  // Commits are newest-first; the rewrite range spans from the oldest edited
  // commit up to the tip, so its length is (max dirty index + 1).
  const rewriteCount = useMemo(() => {
    let maxIdx = -1;
    commits.forEach((c, i) => {
      if (edits[c.hash]) maxIdx = Math.max(maxIdx, i);
    });
    return maxIdx + 1;
  }, [commits, edits]);

  const pushedWarning =
    selectedBranch?.aheadOfUpstream != null &&
    rewriteCount > selectedBranch.aheadOfUpstream;

  // Signed commits inside the rewrite range (indices 0..rewriteCount) lose
  // their signature unless re-signed.
  const signedInRange = useMemo(
    () => commits.slice(0, rewriteCount).filter((c) => c.signed).length,
    [commits, rewriteCount]
  );

  async function reload(path: string, branchName: string) {
    const [cs, bs] = await Promise.all([
      listCommits(path, branchName, LIMIT),
      listBackups(path),
    ]);
    setCommits(cs);
    setBackups(bs);
  }

  async function doOpen(path: string, wantedBranch?: string) {
    if (!path.trim()) return;
    setBusy(true);
    setError(null);
    setNotice(null);
    try {
      const info = await openRepo(path.trim());
      const brs = await listBranches(info.path);
      const wanted = brs.find((b) => b.name === wantedBranch)?.name;
      const initial =
        wanted ??
        brs.find((b) => b.name === info.branch)?.name ??
        brs.find((b) => b.isHead)?.name ??
        brs[0]?.name ??
        info.branch;
      setRepo(info);
      setPathInput(info.path);
      setBranches(brs);
      setBranch(initial);
      setEdits({});
      setExpanded(null);
      await reload(info.path, initial);
      if (wantedBranch && !wanted) {
        setError(`No local branch "${wantedBranch}" — showing ${initial}`);
      }
    } catch (e) {
      setRepo(null);
      setBranches([]);
      setCommits([]);
      setBackups([]);
      setError(String(e));
    } finally {
      setBusy(false);
    }
  }

  // Open the repo (and branch) passed as arguments, once (StrictMode runs effects twice in dev).
  const openedInitial = useRef(false);
  useEffect(() => {
    if (openedInitial.current) return;
    openedInitial.current = true;
    initialRepo().then((r) => {
      if (r) {
        setPathInput(r.path);
        doOpen(r.path, r.branch ?? undefined);
      }
    });
  }, []);

  async function selectBranch(name: string) {
    if (!repo || name === branch) return;
    setBranch(name);
    setEdits({});
    setExpanded(null);
    setBusy(true);
    setError(null);
    try {
      await reload(repo.path, name);
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  }

  async function browse() {
    const picked = await openDialog({ directory: true, title: "Select a git repository" });
    if (typeof picked === "string") {
      setPathInput(picked);
      await doOpen(picked);
    }
  }

  function onField(
    hash: string,
    field: EditableField,
    value: string,
    original: string
  ) {
    setEdits((prev) => {
      const cur: CommitEdit = { ...(prev[hash] ?? { hash }) };
      if (value === original) {
        delete cur[field];
      } else {
        cur[field] = value;
      }
      const hasEdits = Object.keys(cur).some((k) => k !== "hash");
      const next = { ...prev };
      if (hasEdits) next[hash] = cur;
      else delete next[hash];
      return next;
    });
  }

  function resetRow(hash: string) {
    setEdits((prev) => {
      const next = { ...prev };
      delete next[hash];
      return next;
    });
  }

  function toggle(hash: string) {
    setExpanded((cur) => (cur === hash ? null : hash));
  }

  // Merge the computed bulk result into the edit map, normalizing against each
  // commit's original value so a replacement back to the original clears it.
  function stageBulk() {
    if (bulkResult.error || bulkResult.matchedCommits === 0) return;
    setEdits((prev) => {
      const next = { ...prev };
      for (const [hash, fields] of Object.entries(bulkResult.edits)) {
        const c = commitByHash[hash];
        if (!c) continue;
        const cur: CommitEdit = { ...(next[hash] ?? { hash }) };
        for (const field of Object.keys(fields) as BulkField[]) {
          const value = fields[field]!;
          const original =
            field === "message"
              ? fullMessage(c)
              : c[field as Exclude<BulkField, "message">];
          if (value === original) delete cur[field];
          else cur[field] = value;
        }
        const hasEdits = Object.keys(cur).some((k) => k !== "hash");
        if (hasEdits) next[hash] = cur;
        else delete next[hash];
      }
      return next;
    });
    setNotice(
      `Staged ${bulkResult.replacements} replacement` +
        `${bulkResult.replacements === 1 ? "" : "s"} across ${bulkResult.matchedCommits} ` +
        `commit${bulkResult.matchedCommits === 1 ? "" : "s"}. Review the highlighted rows, then apply.`
    );
  }

  async function startApply() {
    if (!repo) return;
    setBusy(true);
    setError(null);
    try {
      const changes = await previewEdits(repo.path, branch, Object.values(edits));
      setConfirm(changes);
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  }

  async function confirmApply() {
    if (!repo) return;
    setBusy(true);
    setError(null);
    try {
      const res = await applyEdits(repo.path, branch, Object.values(edits), sign, resign);
      setConfirm(null);
      setEdits({});
      setExpanded(null);
      setNotice(
        `🔪 Stabbed ${res.rewrittenCount} commit${res.rewrittenCount === 1 ? "" : "s"} on ${branch}. ` +
          `New tip ${res.newHead.slice(0, 8)} · backup ${res.backupRef}` +
          (sign ? " · signed with a git-knife note" : "")
      );
      const info = await openRepo(repo.path);
      setRepo(info);
      setBranches(await listBranches(info.path));
      await reload(info.path, branch);
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  }

  async function restore(refName: string) {
    if (!repo) return;
    const ok = await ask(
      "Restore this backup? This runs `git reset --hard` and discards the " +
        "current branch tip (and any uncommitted changes).",
      { title: "Restore backup", kind: "warning" }
    );
    if (!ok) return;
    setBusy(true);
    setError(null);
    try {
      await restoreBackup(repo.path, refName);
      setEdits({});
      setExpanded(null);
      setNotice("Restored from backup.");
      const info = await openRepo(repo.path);
      setRepo(info);
      setBranches(await listBranches(info.path));
      await reload(info.path, branch);
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="app">
      <header className="topbar">
        <div className="brand">
          <span className="logo">🔪</span>
          <span>git-knife</span>
        </div>
        <div className="repo-open">
          <input
            className="path"
            placeholder="/path/to/repository"
            value={pathInput}
            spellCheck={false}
            onChange={(e) => setPathInput(e.target.value)}
            onKeyDown={(e) => e.key === "Enter" && doOpen(pathInput)}
          />
          <button className="ghost" onClick={browse} disabled={busy}>
            Browse…
          </button>
          <button className="primary" onClick={() => doOpen(pathInput)} disabled={busy}>
            Open
          </button>
        </div>
      </header>

      {repo && (
        <div className="repo-info">
          <span>
            <span className="muted">branch</span>{" "}
            <select
              className="branch-select"
              value={branch}
              disabled={busy || branches.length === 0}
              onChange={(e) => selectBranch(e.target.value)}
            >
              {branches.map((b) => (
                <option key={b.name} value={b.name}>
                  {b.name}
                  {b.isHead ? " ✓" : ""}
                </option>
              ))}
            </select>
          </span>
          <span>
            <span className="muted">tip</span>{" "}
            <code>{(selectedBranch?.head ?? repo.head).slice(0, 8)}</code>
          </span>
          <span>
            <span className="muted">upstream</span>{" "}
            {selectedBranch?.upstream ? (
              <>
                <code>{selectedBranch.upstream}</code>
                {selectedBranch.aheadOfUpstream != null && (
                  <span className="muted"> ({selectedBranch.aheadOfUpstream} ahead)</span>
                )}
              </>
            ) : (
              <span className="muted">none</span>
            )}
          </span>
          <label
            className="chk sign-toggle"
            title="Attach a transparent git-knife note (refs/notes/git-knife) to rewritten commits. Read it with: git notes --ref=git-knife show <commit>"
          >
            <input
              type="checkbox"
              checked={sign}
              onChange={(e) => toggleSign(e.target.checked)}
            />
            🔪 signature note
          </label>
        </div>
      )}

      {error && (
        <div className="banner error" onClick={() => setError(null)}>
          {error}
        </div>
      )}
      {notice && (
        <div className="banner notice" onClick={() => setNotice(null)}>
          {notice}
        </div>
      )}

      <main className="content">
        {!repo ? (
          <div className="empty">
            <h1>Stab your git history into shape 🔪</h1>
            <p className="muted">
              Open a repository, pick a commit, and rewrite its message, author,
              and dates — cleanly. Every stab is backed up first and never touches
              your files, only the metadata.
            </p>
          </div>
        ) : (
          <div className="workspace">
            <div className="left-col">
              <div className="toolbar">
                <button
                  className={bulkOpen ? "primary" : "ghost"}
                  onClick={() => setBulkOpen((o) => !o)}
                >
                  {bulkOpen ? "Close bulk edit" : "Bulk find & replace"}
                </button>
                <span className="muted small">{commits.length} commits loaded</span>
              </div>

              {bulkOpen && (
                <BulkPanel
                  spec={bulk}
                  result={bulkResult}
                  busy={busy}
                  onChange={(patch) => setBulk((s) => ({ ...s, ...patch }))}
                  onStage={stageBulk}
                  onClose={() => setBulkOpen(false)}
                />
              )}

              <div className="table-wrap">
                {commits.length === 0 ? (
                  <p className="muted">No commits found.</p>
                ) : (
                  <CommitTable
                    commits={commits}
                    edits={edits}
                    expanded={expanded}
                    onToggle={toggle}
                    onField={onField}
                    onResetRow={resetRow}
                  />
                )}
              </div>
            </div>
            <BackupPanel backups={backups} busy={busy} onRestore={restore} />
          </div>
        )}
      </main>

      <ApplyBar
        dirtyCount={dirtyCount}
        rewriteCount={rewriteCount}
        pushedWarning={!!pushedWarning}
        signedCount={signedInRange}
        resign={resign}
        onToggleResign={toggleResign}
        busy={busy}
        onApply={startApply}
        onDiscard={() => setEdits({})}
      />

      {confirm && (
        <ConfirmDialog
          changes={confirm}
          busy={busy}
          onConfirm={confirmApply}
          onCancel={() => setConfirm(null)}
        />
      )}
    </div>
  );
}
