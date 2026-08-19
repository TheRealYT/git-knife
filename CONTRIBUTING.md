# Contributing to git-knife

Thank you for your interest in contributing! This document covers everything you
need to get up and running.

---

## Prerequisites

| Tool | Version | Notes |
|---|---|---|
| **Rust** | stable (≥ 1.75) | Install via [rustup](https://rustup.rs/) |
| **Node.js** | ≥ 18 | LTS recommended |
| **pnpm** | ≥ 9 | `corepack enable && corepack prepare pnpm@latest --activate` |
| **Tauri CLI** | `@tauri-apps/cli` v2 | Installed as devDependency |
| **System deps** | Platform-specific | See [Tauri prerequisites](https://v2.tauri.app/start/prerequisites/) |

## Dev Setup

```bash
# Clone the repository
git clone https://github.com/k0shir0/git-knife.git
cd git-knife

# Install frontend dependencies
pnpm install

# Run in development mode (starts Vite dev server + Tauri backend)
pnpm tauri dev

# Build a release binary
pnpm tauri build
```

## Project Structure

```
git-knife/
├── src/                    # React + TypeScript frontend
│   ├── components/         # UI components (CommitTable, BulkPanel, etc.)
│   ├── bulk.ts             # Bulk find & replace engine
│   ├── api.ts              # Tauri IPC wrappers
│   ├── types.ts            # Shared TypeScript types
│   └── bulk.test.ts        # Vitest unit tests
├── src-tauri/              # Rust backend (Tauri)
│   ├── src/
│   │   ├── main.rs         # Entry point & command registration
│   │   ├── git.rs          # Git CLI wrapper
│   │   ├── commits.rs      # Commit log parsing
│   │   ├── rewrite.rs      # Core rewrite engine
│   │   ├── backup.rs       # Backup ref management
│   │   └── lock.rs         # Advisory repository locking
│   ├── Cargo.toml
│   └── tauri.conf.json
├── ARCHITECTURE.md         # System design & data flow
└── README.md
```

## Running Tests

### Rust (backend)

```bash
cd src-tauri
cargo test
```

All unit tests live in `#[cfg(test)]` modules colocated with their source files.
Current test coverage:

- `rewrite.rs` — `pick`, `compute_rewrite_set` (linear + merge DAGs)
- `commits.rs` — `find_subslice`, `parse_batch_signed`
- `backup.rs` — `parse_backup_line`
- `lock.rs` — `RepoLock` acquire/release/contention

### TypeScript (frontend)

```bash
pnpm test          # Run Vitest in single-run mode
pnpm test -- --ui  # Run with interactive UI
```

Current test coverage:

- `bulk.test.ts` — `escapeRegExp`, `checkRegexSafety`, `effectiveValue`,
  `computeBulk` (literal, regex, case sensitivity, ReDoS rejection, composition
  with pending edits, multi-field, `$`-literal handling)

### Full Build Check

```bash
pnpm build                   # tsc && vite build (frontend)
cd src-tauri && cargo check  # Rust type checking
```

## Code Style

- **Rust**: Follow standard `rustfmt` formatting. Run `cargo fmt` before
  committing.
- **TypeScript**: No separate linter is configured; the `strict` tsconfig
  catches most issues at compile time. Keep files small and functions pure where
  possible.

## PR Checklist

Before submitting a pull request, please verify:

- [ ] `cargo test` passes with 0 failures and 0 new warnings
- [ ] `pnpm test` passes all Vitest suites
- [ ] `pnpm build` (`tsc && vite build`) completes without errors
- [ ] `cargo fmt -- --check` reports no formatting issues
- [ ] New code has appropriate doc comments (`///` for Rust, JSDoc for TS)
- [ ] If adding a new IPC command: update `ARCHITECTURE.md` IPC contract table
- [ ] If modifying the rewrite algorithm: add or update unit tests in
  `rewrite.rs`
- [ ] Commit messages follow
  [Conventional Commits](https://www.conventionalcommits.org/) format:
  `feat:`, `fix:`, `refactor:`, `docs:`, `test:`, `chore:`

## Commit Format

```
type(scope): short description

Optional longer body explaining the why, not the what.
```

Examples:
```
fix(rewrite): preserve merge parents during DAG rebuild
refactor(lock): remove unused path field from RepoLock
test(bulk): add ReDoS pattern rejection tests
docs: add ARCHITECTURE.md with data flow diagrams
```

## Security

If you discover a security vulnerability, please **do not** open a public issue.
Instead, email the maintainer directly. See the project README for contact
information.
