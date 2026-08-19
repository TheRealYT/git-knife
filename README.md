# git-knife 🔪

*Stab your git history into shape — every commit's message, author, and dates,
edited like a table.*

![git-knife screenshot](docs/screenshot.png)

A clean desktop GUI for editing git commit metadata directly — **message,
author date, committer date, author name/email**.

Existing GUIs (GitKraken, Sublime Merge, Fork, lazygit) reword and reorder well
but treat commit **dates** as effectively immutable and don't expose committer
date / author identity for arbitrary commits. The tools that *can* rewrite that
metadata (`git-filter-repo`, `git rebase` env tricks, `git commit-tree`) have no
GUI. git-knife fills that gap.

It never reimplements git — it shells out to the system `git` CLI and rebuilds
commits with `git commit-tree`, reusing each commit's original tree so **file
contents are provably never changed**.

## How it compares

| Tool | Clean GUI | Reword msg | Reorder / squash / drop | Edit **author date** | Edit **committer date** | Edit author/email | Bulk find & replace (regex) |
|------|:--:|:--:|:--:|:--:|:--:|:--:|:--:|
| **git-knife** | ✅ | ✅ | 🚧 planned | ✅ | ✅ | ✅ | ✅ |
| GitKraken | ✅ | ✅ | ✅ | ⚠️ amend-only | ❌ | ⚠️ | ❌ |
| Sublime Merge | ✅ | ✅ | ✅ | ❌ | ❌ | ⚠️ amend | ❌ |
| Fork | ✅ | ✅ | ✅ | ❌ | ❌ | ⚠️ | ❌ |
| SmartGit | ✅ | ✅ | ✅ | ⚠️ | ❌ | ✅ | ❌ |
| git-cola | ◐ | ✅ | ✅ | ✅ | ❌ | ✅ | ❌ |
| lazygit (TUI) | ◐ TUI | ✅ | ✅ | ❌ | ❌ | ⚠️ | ❌ |
| git-filter-repo (CLI) | ❌ | ✅ | via callback | ✅ | ✅ | ✅ | ✅ |

Legend: ✅ first-class · ⚠️ possible but awkward/limited · ◐ dated or terminal UI · ❌ not supported · 🚧 planned.

The polished GUIs reword and reorder well but treat commit **dates** — especially
the committer date — as effectively immutable, and none offer a bulk regex pass
over author identity. The tools that *can* rewrite that metadata have no GUI.
git-knife is the intersection: a clean GUI that edits every field, in bulk, safely.

## Status (MVP)

- ✅ Open a repo and pick **any local branch** to view/edit — by its ref, no checkout, so your working tree is never touched
- ✅ Edit message / author name+email / author date / committer date / committer name+email
- ✅ Bulk find & replace across those text fields, literal or regex (great for fixing a wrong email everywhere)
- ✅ Preview every change before applying
- ✅ Automatic backup ref before each rewrite + one-click restore
- ✅ Warns when a rewrite would touch already-pushed history
- ✅ Signed-commit aware — badges signed commits, warns a rewrite strips signatures, and can **re-sign** with your key
- ✅ Edit across merges — rebuilds the full commit graph, preserving each merge's parents
- ⛔ Not yet: reorder / squash / drop, staging/branches/remotes

## Requirements

- **git** (2.x)
- **Node.js** + **pnpm** (`corepack enable pnpm`, or `npm i -g pnpm`)
- **Rust** (stable) — install via <https://rustup.rs>
- Linux system deps for Tauri v2: `webkit2gtk-4.1`, `libgtk-3`, `libayatana-appindicator3`, `librsvg2`
  (Debian/Ubuntu: `sudo apt install libwebkit2gtk-4.1-dev build-essential curl wget file libxdo-dev libssl-dev libayatana-appindicator3-dev librsvg2-dev`)

## Run (dev)

```bash
pnpm install
pnpm tauri dev
```

The first `cargo` build downloads and compiles the Tauri crates (a few minutes).

## Build a bundle

```bash
pnpm tauri build
```

> Packaging (`tauri build`) needs app icons. They're already committed under
> `src-tauri/icons/`; regenerate from any square PNG with
> `pnpm tauri icon path/to/icon.png`. Dev runs don't need them.

### Automated releases (GitHub Actions)

`.github/workflows/release.yml` builds native installers for **macOS, Linux, and
Windows** with [`tauri-action`](https://github.com/tauri-apps/tauri-action) and
attaches them to a draft GitHub Release. Cut a release by pushing a tag:

```bash
git tag v0.1.0
git push origin v0.1.0
```

(or trigger it manually from the repo's **Actions** tab). No code signing is set
up yet, so macOS/Windows builds are unsigned — fine for early testers.

## Editing history & pushing

### Edit a commit

1. **Open** a repository (Browse… or paste the path), then pick a **branch**
   from the dropdown — git-knife edits it by its ref, without checking it out,
   so your working tree and current checkout stay exactly as they are.
2. Click any commit to expand its editor (editing commits across merges rebuilds
   the full DAG, preserving each merge commit's parents).
3. Change the message, author/committer name, email, or dates. Edited rows are
   highlighted; the date fields keep the commit's original UTC offset.
4. Click **Review & apply**, check the old → new preview, and confirm.

git-knife rewrites **only your local branch**. It never contacts a remote and
never pushes for you — pushing is always your explicit step.

### Bulk find & replace

Click **Bulk find & replace** above the commit table to change text across many
commits at once:

- Pick which fields to target (message, author/committer name, author/committer
  email — any combination).
- Enter **Find** / **Replace**. Toggle **Regex** for pattern matching with `$1`
  backreferences, or leave it off for a literal search. **Case-sensitive** is on
  by default.
- The panel live-counts matching commits and replacements. Click **Stage edits**
  to turn them into highlighted rows, then **Review & apply** as usual.

Example — move every commit from an old email to a new one: target *Author
email* + *Committer email*, find `old@example.com`, replace `new@example.com`.
Merge commits are included, and successive passes compose.

### Push a rewrite

Editing a commit changes its hash **and the hash of every commit after it**, so
your local branch and the remote have diverged. A normal `git push` is rejected
as *non-fast-forward*. Push with a lease:

```bash
git push --force-with-lease origin <branch>
```

`--force-with-lease` refuses the push if the remote moved since your last fetch,
so you can't silently clobber a teammate's commits. Prefer it over plain
`--force`, which skips that safety check.

> git-knife shows a **"rewrites pushed history"** warning when your edit reaches
> into commits that already exist on the upstream. If you can, edit only unpushed
> commits — rewriting shared history forces everyone else to re-sync.

### After rewriting shared history

Anyone who already pulled the old commits now has divergent history. Each of them
re-syncs their local branch to the new remote state:

```bash
git fetch origin
git reset --hard origin/<branch>   # discards local-only commits — coordinate first
```

### Undo a rewrite

- **In-app:** the **Backups** panel restores the pre-rewrite tip in one click.
- **From the CLI:** every apply saved a backup ref —

  ```bash
  git for-each-ref refs/knife-backup      # find the pre-rewrite tip
  git reset --hard <backup-ref-or-hash>   # move the branch back
  ```

  `git reflog` also lists the old tip. If you already force-pushed, restore
  locally and then `git push --force-with-lease` again.

### Signed commits

Rewriting a commit changes its hash, which **invalidates any GPG/SSH signature**
on it (a raised concern on HN). git-knife detects signed commits by their raw
`gpgsig` header — independent of verification, so it catches SSH signatures even
without an `allowedSignersFile` — and:

- **badges** them `signed` in the table,
- **warns** in the apply bar how many signed commits a rewrite would strip,
- offers a **re-sign** toggle that re-signs the rebuilt commits with your
  configured key (`user.signingkey` / `gpg.format`). If re-signing is on but no
  key is configured, the apply fails safely before touching any ref.

### Signature note (transparent)

By default git-knife attaches a small, **disclosed** note to each rewritten tip
commit, on its own notes ref so it never touches your regular notes:

```bash
git notes --ref=git-knife show <commit>   # read it
git for-each-ref refs/notes/git-knife      # was this repo edited by git-knife?
```

It's invisible in a normal `git log` (separate ref) but fully discoverable — no
hidden encoding. Toggle it off anytime with the **🔪 signature note** checkbox in
the app (the setting is remembered). To strip it from a repo entirely:

```bash
git update-ref -d refs/notes/git-knife
```

## How it works

- `src-tauri/src/git.rs` — the only place that spawns `git`.
- `commits.rs` — `open_repo`, `list_commits` (NUL/record-separator parsing).
- `rewrite.rs` — `preview_edits` + `apply_edits`: rebuilds the chain from the
  earliest edited commit to the tip via `commit-tree`, then saves a backup ref
  and moves the branch with a compare-and-swap on the old tip.
- `backup.rs` — lists `refs/knife-backup/*` and restores via `git reset --hard`.

The rewrite strategy is validated at the git level by
`scratchpad/verify_engine.sh` (reproduces the exact commit-tree flow and asserts
the content diff is empty).

## Safety

Every apply creates `refs/knife-backup/<branch>/<epoch>` pointing at the old tip
before touching anything. Nothing is force-deleted; restore is always available
from the Backups panel.
