# Implementation Plan: git-knife Priority Maintenance Backlog Execution

Execute the full Phase 4 maintenance backlog for `git-knife` following an E2E test-driven refactoring protocol, sandboxed container verification, and single-author commit compliance (`k0shir0`).

---

## User Review Required

> [!IMPORTANT]
> All GitHub Actions in `.github/workflows/release.yml` will be pinned to verified, immutable commit SHAs with version comments (e.g. `actions/checkout@11bd71901bbe5b1630ceea73d27597364c9af683 # v4.2.2`).

> [!NOTE]
> All commits will strictly adhere to the `k0shir0` identity with no AI co-author attribution, secondary sign-offs, or bot metadata.

---

## Proposed Changes

### 1. Supply Chain Hardening & Documentation

#### [MODIFY] [release.yml](file:///C:/Users/hardc/.gemini/antigravity/scratch/git-knife/.github/workflows/release.yml)
- Pin all 6 GitHub Actions to their exact commit SHAs:
  - `actions/checkout@11bd71901bbe5b1630ceea73d27597364c9af683 # v4.2.2`
  - `pnpm/action-setup@0c17529a66aca453f9227af23103ed11469b1e47 # v4.0.0`
  - `actions/setup-node@1d0ff469b7ec7b3cb9d8673fde0c81c44821de2a # v4.2.0`
  - `dtolnay/rust-toolchain@6c977a6ca4077a0ceb28ffbe03f59d46e9ac8772 # stable`
  - `swatinem/rust-cache@400e7407cfd7a091e5fbb6afec01ec146c432b7c # v2.7.7`
  - `tauri-apps/tauri-action@2773e489dd94df5e87641131f67916b46ef3a8d5 # v0.5.15`

#### [MODIFY] [README.md](file:///C:/Users/hardc/.gemini/antigravity/scratch/git-knife/README.md)
- Ensure all merge references accurately describe that git-knife rebuilds the full DAG across merge commits, faithfully preserving each merge's parents without ever skipping or mangling them.
- Update feature comparison and roadmap clarity.

---

### 2. Frontend Security & Performance

#### [MODIFY] [bulk.ts](file:///C:/Users/hardc/.gemini/antigravity/scratch/git-knife/src/bulk.ts)
- Implement ReDoS protection:
  - Add regex validation and bounded execution time / complexity safeguards before executing user-supplied regexes across commit datasets.
  - Return clear, user-friendly error messages in `BulkResult.error` when a catastrophic backtracking or overly complex expression is detected.

#### [MODIFY] [CommitTable.tsx](file:///C:/Users/hardc/.gemini/antigravity/scratch/git-knife/src/components/CommitTable.tsx)
- Implement virtualized list rendering (windowing) for the commit table:
  - Render only the visible window of rows (plus overscan buffer) to maintain 60fps scrolling on repos with thousands of commits.
  - Support expandable editor rows with dynamic height calculations.
  - Preserve exact layout, column alignment, sticky table header, and dirty/signed/merge badges.

---

### 3. Rust Backend Refactoring & Concurrency Safety

#### [MODIFY] [Cargo.toml](file:///C:/Users/hardc/.gemini/antigravity/scratch/git-knife/src-tauri/Cargo.toml)
- Add advisory locking crate (`fs2 = "0.4"` or `fd-lock = "4.0"` / cross-platform file lock) to secure concurrent repository access.

#### [NEW] [lock.rs](file:///C:/Users/hardc/.gemini/antigravity/scratch/git-knife/src-tauri/src/lock.rs)
- Implement `RepoLock` RAII advisory lock on `.git/knife.lock`:
  - Acquires exclusive lock during `apply_edits` and `restore_backup`.
  - Automatically releases on drop.
  - Returns informative error if another git-knife or concurrent process holds the lock.

#### [MODIFY] [rewrite.rs](file:///C:/Users/hardc/.gemini/antigravity/scratch/git-knife/src-tauri/src/rewrite.rs)
- Extract the 10-parameter `commit_tree` arguments into `CommitTreeConfig`.
- Decompose `apply_edits` into three modular, testable functions:
  1. `compute_rewrite_set(chain: &[RawCommit], edit_map: &HashMap<&str, &CommitEdit>) -> HashSet<String>`
  2. `rebuild_chain(repo: &str, chain: &[RawCommit], rewrite_set: &HashSet<String>, edit_map: &HashMap<&str, &CommitEdit>, resign: bool) -> Result<HashMap<String, String>, String>`
  3. `update_refs(repo: &str, branch: &str, old_head: &str, new_head: &str, rewritten_count: u32, sign: bool) -> Result<ApplyResult, String>`
- Integrate `RepoLock` into `apply_edits`.
- Add `#[cfg(test)]` unit test suite covering `pick`, `compute_rewrite_set`, `rebuild_chain`, and `update_refs`.

#### [MODIFY] [commits.rs](file:///C:/Users/hardc/.gemini/antigravity/scratch/git-knife/src-tauri/src/commits.rs)
- Add `#[cfg(test)]` unit tests for `find_subslice`, `signed_set` (detecting `gpgsig` headers on signed vs unsigned commits, multi-byte content, missing hashes), and `open_repo`/`list_commits` parent parsing.

#### [MODIFY] [backup.rs](file:///C:/Users/hardc/.gemini/antigravity/scratch/git-knife/src-tauri/src/backup.rs)
- Integrate `RepoLock` into `restore_backup`.
- Add `#[cfg(test)]` unit tests for backup listing, timestamp sorting, and ref parsing.

#### [MODIFY] [main.rs](file:///C:/Users/hardc/.gemini/antigravity/scratch/git-knife/src-tauri/src/main.rs)
- Register `mod lock;`.

---

### 4. Testing Infrastructure & Contributor Documentation

#### [MODIFY] [package.json](file:///C:/Users/hardc/.gemini/antigravity/scratch/git-knife/package.json)
- Add `vitest` to `devDependencies` and add `"test": "vitest run"` script.

#### [NEW] [bulk.test.ts](file:///C:/Users/hardc/.gemini/antigravity/scratch/git-knife/src/bulk.test.ts)
- Comprehensive Vitest unit test suite covering:
  - `computeBulk`: literal replace, regex replace, regex flags, dollar sign escaping in literal mode, ReDoS prevention & timeout, multi-field edits, and edit composition.
  - `effectiveValue`: pending overrides vs original values.
  - `escapeRegExp`: regex special character escaping.
  - `fullMessage`: commit message reconstructions.

#### [NEW] [ARCHITECTURE.md](file:///C:/Users/hardc/.gemini/antigravity/scratch/git-knife/ARCHITECTURE.md)
- Complete architectural documentation:
  - High-level system architecture and Tauri v2 IPC bridge.
  - Mermaid data flow diagrams.
  - Rewrite engine DAG algorithm (topological traversal, descendant closure, parent remapping, tree hash preservation guarantee).
  - Security model (no shell interpolation, verification-independent signature detection, advisory locking).
  - Backup & restoration subsystem (`refs/knife-backup/*` and `refs/notes/git-knife`).

#### [NEW] [CONTRIBUTING.md](file:///C:/Users/hardc/.gemini/antigravity/scratch/git-knife/CONTRIBUTING.md)
- Development setup guide for macOS, Linux, and Windows.
- Build, test, and lint commands (`cargo test`, `pnpm test`, `pnpm build`, `cargo clippy`).
- PR submission checklist and code quality standards.

---

## Verification Plan

### Automated Tests
1. **Rust Unit Tests**:
   - `cargo test -- --nocapture` inside isolated container sandbox.
   - Verifies `pick`, `signed_set`, `compute_rewrite_set`, `rebuild_chain`, `RepoLock`, and backup parsing.
2. **TypeScript Unit Tests**:
   - `pnpm test` (Vitest).
   - Verifies `computeBulk`, `effectiveValue`, ReDoS prevention, and literal/regex replacements.
3. **TypeScript Typecheck & Production Build**:
   - `pnpm build` (`tsc && vite build`).
4. **Rust Clippy & Linter Check**:
   - `cargo clippy -- -D warnings`.
5. **E2E & Integration Regression Suite**:
   - Execute the 20-point integration test harness verifying tree preservation, merge parent preservation, root commit handling, unicode handling, backup restoration, crash resilience, and advisory locking.

### Manual / Sandbox Verification
- Run the full test suite in `HermesSandbox`.
- Verify clean git status and format commit log with single author `k0shir0`.
