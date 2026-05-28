# gdl — git diff list, in a terminal, with the rich VSCode SC + diff-editor experience

A net-new Rust workspace at `~/gdl` (GitHub: `npiesco/gdl`) that surfaces VSCode-Source-Control-panel + diff-editor parity in pure terminal output (ANSI), and exposes **the exact same rendered bytes** through both a CLI binary (`gdl`) and an MCP server (`gdl-mcp`). One function per operation, two thin façades — the sessql DRY contract.

---

## 1. Problem statement

When working over SSH from a beefier dev machine, VSCode (an Electron app) is unavailable and bloated. The most-missed thing is the **Source Control panel file list** and the **diff editor** — the at-a-glance "what's actually changed" view. Existing terminal tools either need a TUI (lazygit, gitui) or only render diff *content* (delta, difftastic) — none ship the bare **file-list-with-status-badges-and-+/-counts** experience VSCode SC panel gives, plus a per-file syntax-highlighted side-by-side diff, as a single non-interactive command whose output is byte-identical across CLI and MCP.

`gdl` ships exactly that and nothing else.

---

## 2. Methodology (TDD discipline)

Every feature follows this loop. **Strict**, no shortcuts.

1. **Red** — Write a *real* integration test that drives the real code path against real I/O. Initialize a real git repo with `gix::open`/`gix::init`, write real files, stage/modify/delete real bytes, call the real public API, assert on real returned data.
2. **Present approach** — Explain how the green will be implemented. Wait for sign-off.
3. **Green** — Implement until the test passes.
4. **Regression** — `cargo test --workspace`.
5. **Lint** — `cargo fmt --all` + `cargo clippy --workspace --all-targets -- -D warnings`. Fix all.
6. **Regression again** — `cargo test --workspace` after lint fixes.
7. **Rebuild** — `make release` to produce the release binaries without invoking a standalone `cargo build` command directly.

If a step fails, add structured `tracing` logging around the troublesome code path and iterate until it passes; do not skip steps or relax the test.

### Test discipline — what is and is not a test

A test that opens a `.rs` file with `fs::read_to_string` and asserts a substring is present/absent is a **tripwire, not a test**.

**NEVER write a test that:**
- Reads source files as strings and asserts on substrings (`src.contains(...)`, brace-balanced body grepping, etc.).
- Asserts on the existence, structure, signature, or naming of an API without invoking it.
- Mocks, stubs, fakes, or otherwise replaces the code path it claims to cover.
- Uses `#[ignore]`, `todo!()`, `unimplemented!()`, "for now", "later", or any deferred assertion.

**EVERY test must:**
- Drive the actual code path that contains the behavior.
- Use the real types, the real I/O, the real timing (real `gix` repos in `tempfile::TempDir`, real `assert_cmd::Command` binary invocations, real `rmcp` JSON-RPC client/server round-trips over stdio).
- Fail RED for the actual runtime reason, not because a string is present in a file.
- Turn GREEN only when the runtime mechanism is correct.

**Banned in source & tests:** `todo!`, `unimplemented!`, `#[ignore]`, "TODO", "FOR NOW", "LATER", "DEFER", mock objects, stub returns. Tests are integration tests rooted in `crates/<crate>/tests/`. **No unit-test-only proofs of behavior** — every behavioral claim must be reachable from the public API and demonstrated end-to-end.

### 2.1 Cross-repo support and dogfood discipline

**First-class `--repo <PATH>` everywhere.** `gdl` must operate on any git repo, not just `$PWD`. The flag flows:

| Surface | Shape |
|---|---|
| CLI | `gdl --repo <PATH> [subcommand]` — top-level flag on the `clap::Parser` root, applies to every subcommand. Defaults to `std::env::current_dir()?` when omitted. |
| MCP tool params | Every tool input struct has a `repo_path: Option<String>` field with `#[schemars(description = "...")]`. `None` ⇒ daemon's cwd. |
| Core helpers | Every `*_to_string` helper takes `repo: &gix::Repository` (already opened by caller). The `repo_path` plumbing lives only in the two thin façades (CLI/MCP); the core stays path-agnostic. |

`gdl_core::open(path)` resolves `path` through `gix::ThreadSafeRepository::open` so it accepts:
- absolute paths (`/abs/path/to/repo`),
- relative paths (`../sibling-repo`),
- bare repos and worktrees,
- repos discovered upward from a subdirectory (`gix::discover` fallback when the literal path isn't a repo root).

**Dogfood on the gdl repo itself, end-to-end.** Every CLI and MCP integration test ships in two flavors:

1. **Fixture flavor** — `tempfile::tempdir()` + `gix::init` + scripted file mutations. Deterministic. Owns the assertion on exact byte output.
2. **Dogfood flavor** — open `~/gdl` itself (located via `let workspace_root = env!("CARGO_MANIFEST_DIR")` walked up to the workspace root). Asserts only **structural invariants** that hold for any real repo: command exits 0, output is non-empty, every parsed `GdlEntry.path` is a valid `Path`, ANSI variant contains at least one `\x1b[` escape when stdout is forced to a TTY (via `IsTerminal` stub at test time), CLI ↔ MCP byte-equality (see Feature 13) holds.

The dogfood flavor catches integration drift that tempdir fixtures miss — real `.gitignore`, real submodules-or-not state, real CRLF settings, real ignore patterns from a `.git/info/exclude`. **It runs locally and in CI on every commit**, so a refactor that breaks rendering on the project's own repo fails the build.

**Why path-required (not "just use $PWD"):** the user explicitly works over SSH with build/dev split across machines and frequently invokes tools from a directory that is **not** the repo of interest. `--repo` is a hard requirement, not a nice-to-have.

### 2.2 Render contract — explicit options, no ambient state

**The byte-equality invariant (Feature 13) is the contract.** It only holds if both façades pass *identical, explicit* render options to the shared helper. Therefore the core helpers MUST NOT inspect ambient process state (TTY detection, `COLUMNS`, `NO_COLOR`, `TERM`, `LANG`). All such resolution happens in the two thin façades **before** calling the helper.

```rust
// gdl-format::types
pub enum OutputFormat {
    Plain,
    Ansi,
    Json,
}

pub enum ColorPolicy {
    Never,    // explicit no-color
    Always,   // explicit force-color
}

pub enum StatusView {
    Full,        // section headers + badge + path + +N/-N
    PathsOnly,   // newline-joined paths only
}

pub struct RenderOptions {
    pub format: OutputFormat,
    pub color: ColorPolicy,   // resolved by caller; helper does not auto-detect
    pub width: u16,           // explicit terminal width; helper does not auto-detect
    pub view: StatusView,
}
```

**Façade resolution rules (CLI):**
- `--color always` ⇒ `ColorPolicy::Always`; `--color never` ⇒ `ColorPolicy::Never`; default `auto` resolves to `Always` iff `stdout` is a TTY AND `NO_COLOR` is unset, else `Never`.
- `--width N` ⇒ explicit; default reads `crossterm::terminal::size()`; falls back to `80` when not a TTY.
- `--paths-only` ⇒ `StatusView::PathsOnly`; default `Full`.
- `--format ansi|plain|json` ⇒ `OutputFormat::*`; default: `Ansi` when `ColorPolicy::Always`, else `Plain`.

**Façade resolution rules (MCP):**
- Tool input struct has `color: Option<ColorPolicy>` (default `Never`), `width: Option<u16>` (default `80`), `view: Option<StatusView>` (default `Full`), `format: Option<OutputFormat>` (default `Plain`).
- Every default is deterministic and platform-independent — the MCP surface NEVER sniffs the daemon's environment.

**Byte-equality tests in Feature 13** pass identical `RenderOptions` to both façades. The tests cover every meaningful combination: `(Plain, Never)`, `(Ansi, Always)`, `(Json, Never)`, `(PathsOnly, Never)`. Equality cannot be "trivially true because both happen to be Plain".

### 2.3 Status entry model — section is a first-class field

Git status is two-dimensional (index status × worktree status); the VSCode SC panel renders three sections (`Staged Changes`, `Changes`, `Merge Changes`). A single tracked path with both staged and unstaged edits MUST produce two `GdlEntry` rows — one per section. The model:

```rust
// gdl-core::types
pub enum ChangeSection {
    Staged,        // index differs from HEAD
    WorkingTree,   // worktree differs from index
    Untracked,     // present in worktree, absent from index
    Conflicted,    // unmerged stages present
}

pub enum ChangeType {
    Modified,
    Added,
    Deleted,
    Renamed,
    Copied,
    TypeChanged,
    Conflicted,    // pairs with ChangeSection::Conflicted
    Untracked,     // pairs with ChangeSection::Untracked
}

pub struct GdlEntry {
    pub section: ChangeSection,
    pub kind: ChangeType,
    pub path: PathBuf,
    pub old_path: Option<PathBuf>,   // Some for Renamed/Copied
    pub lines_added: u32,
    pub lines_removed: u32,
    pub is_binary: bool,             // suppresses +N/-N counts; renders as `binary`
}
```

**Count rules (tested explicitly in Feature 3):**
| `kind` | `lines_added` | `lines_removed` |
|---|---|---|
| `Untracked` | line count of file (worktree) | 0 |
| `Added` | line count of added blob | 0 |
| `Deleted` | 0 | line count of removed blob |
| `Modified` | Σ `(after.end - after.start)` over hunks | Σ `(before.end - before.start)` over hunks |
| `Renamed` no edit | 0 | 0 |
| `Renamed` + edit | as `Modified` against the old blob | as `Modified` against the old blob |
| `Copied` | as `Modified` against source blob | as `Modified` against source blob |
| `TypeChanged` | line count of new blob | line count of old blob |
| `Conflicted` | 0 | 0 (no count; rendered as `!`) |
| any with `is_binary=true` | 0 | 0 (rendered as `binary`) |

**Deterministic ordering:** before rendering, entries are sorted by `(section as u8, path, kind as u8)`. The Plain/Ansi/Json output is byte-stable regardless of `gix::status` traversal order. Tested in Feature 4.

### 2.4 `gdl diff` baseline semantics

A single path can have different diffs depending on which "area" of git state you compare. The CLI and MCP both expose this explicitly:

| `--area` | Compared | Notes |
|---|---|---|
| `worktree` (default) | worktree vs index | What `git diff` shows by default |
| `staged` | index vs HEAD | What `git diff --staged` shows |
| `head` | worktree vs HEAD | Combined staged+unstaged |

MCP mirrors with `area: Option<DiffArea>` (default `Worktree`). Default is documented and tested. Untracked files: `--area worktree` diffs against `/dev/null` (the empty blob) to show the file's contents as a pure-addition hunk; same for MCP.

---

## 3. What we are leveraging vs. inventing — source-repo citations

All three sibling repos are at `~/` on this machine. Every line cited below is real and used verbatim or as a structural template.

### 3.1 From `~/kagmus` — the diff engine

`gdl-core` reuses kagmus's exact approach to byte-exact hunk computation rather than reinventing it.

| What we leverage | Source | What we keep / what we change |
|---|---|---|
| `gix-diff` recipe as the diff backbone | `~/kagmus/crates/ws-diff/Cargo.toml:14` and `~/kagmus/crates/ws-diff/src/hunks.rs:8-11, 21-26` | **Reuse the public API shape, not the version pin.** Add via `cargo add` only; Cargo resolves the compatible version for this workspace's `rust-version`. |
| `byte_lines` + `InternedInput` + `Algorithm::Histogram` + `diff_with_slider_heuristics` recipe | `~/kagmus/crates/ws-diff/src/hunks.rs:8-11, 21-26, in compute_hunks()` | **Keep verbatim.** This is the gix-diff idiomatic recipe for line-granular hunks with CRLF/non-UTF-8 round-trip. We pull just `hunks.rs::compute_hunks` shape — drop `Hunk` (which carries kagmus-specific `DiffEntryId`/`HunkId`) and replace with our own `Hunk` struct typed for rendering (old_start, old_lines, new_start, new_lines, old_bytes, new_bytes). |
| `ChangeType` enum shape | `~/kagmus/crates/ws-core/src/diff.rs:36-45` (`Created`, `Modified`, `MetadataOnly`, `Deleted`, `DirReplaced`, `Renamed`) | **Reshape.** kagmus models overlay-fs change types. We map to git's natural ones: `Modified`, `Added`, `Deleted`, `Renamed`, `Copied`, `Untracked`, `Conflicted`, `TypeChanged`. Same Display/FromStr pattern (lines 47-74). |
| `classify_entry(rel_path, lower)` pattern | `~/kagmus/crates/ws-core/src/diff.rs` + `~/kagmus/crates/ws-diff/src/classify.rs:8-13` | **Replace.** kagmus classifies by "does lower-dir contain the path"; git classifies by index/worktree status (which `gix::status` already gives us). We will call `gix::status()` and translate, not roll our own tree walk. |
| `hunks_overlap` and `locate_ancestor` | `~/kagmus/crates/ws-diff/src/hunks.rs:86-90, 101-135` | **Skip.** Overlap detection and SHA-anchored hunk relocation are kagmus's per-branch promote workflow needs; `gdl` is read-only render of working-tree state. We do not reimplement these. |
| Scan-tree walker | `~/kagmus/crates/ws-diff/src/scan.rs:12-98` (`collect_files`, OverlayFS whiteout detection) | **Skip.** Specific to OverlayFS upper/lower; git has its own index — we lean on `gix::status`. |

### 3.2 From `~/sessql` — the DRY `*_to_string` contract

`gdl-format` and `gdl-mcp` reproduce the sessql contract structurally.

| What we leverage | Source | How we apply it |
|---|---|---|
| Single source of truth: each operation is **one** `*_to_string(...) -> Result<String, String>` helper | `~/sessql/crates/sessql-cli/src/handlers.rs:195-204` (`search_to_string`), `:209-253` (`find_to_string`), and 20+ siblings up to `:888` | **Keep verbatim shape.** Every `gdl` operation is one `fn name_to_string(args..., fmt: OutputFormat) -> Result<String, String>`. The MCP layer never reshapes. |
| MCP tool body = one-line wrapper over the helper | `~/sessql/crates/sessql-mcp/src/lib.rs:506-516` (`pub async fn search(...)` calls `sessql_cli::search_to_string`) | **Keep verbatim shape.** Every `gdl-mcp` tool body is `match gdl_format::name_to_string(...) { Ok(t) => Ok(tool_success(t)), Err(m) => Ok(tool_error(m)) }`. README §"How each tool body is the same shape" (`~/sessql/README.md:533-542`) is the contract. |
| `OutputFormat` enum with three variants + `format_rows` dispatch | `~/sessql/crates/sessql-core/src/format.rs:5-39` (`OutputFormat::Text|Json|Csv`, `format_rows(&Rows, fmt)`) | **Adapt.** `gdl-format::OutputFormat` = `Ansi` (default for TTY), `Plain` (no color), `Json` (structured). We drop CSV (file-tree shape isn't tabular). |
| MCP `ToolOutputFormat` schema + `From<ToolOutputFormat> for OutputFormat` bridge | `~/sessql/crates/sessql-mcp/src/lib.rs:45-78` | **Keep verbatim.** Same pattern: separate MCP-facing enum with hand-written `JsonSchema` impl + a `From` to the core enum. |
| `pub use` re-export pattern in cli lib.rs | `~/sessql/crates/sessql-cli/src/lib.rs:1-23` | **Keep verbatim.** `gdl-cli` will `pub use gdl_format::*_to_string` so both binaries import from one path. |

### 3.3 From `~/PilotOS` — the CLI surface polish

| What we leverage | Source | How we apply it |
|---|---|---|
| Clap dual-mode binary: subcommand-or-default-action structure | `~/PilotOS/pilot-os/src-tauri/src/cli.rs:31-75` (`#[derive(Parser)] Cli` + `#[derive(Subcommand)] CliCommand { Exec, Prompt, Chat, McpServer }`) | **Adapt.** `gdl` clap shape = `gdl` (default = status), `gdl status`, `gdl diff <path>`, `gdl mcp-server`. |
| `run_cli` dispatch function returning exit code | `~/PilotOS/pilot-os/src-tauri/src/cli.rs:113-180` | **Keep verbatim shape.** `gdl-cli::run(cli) -> i32`. |
| crossterm-based renderer scaffolding (Color enum, ColorTheme struct with named slots, `IsTerminal` detection) | `~/PilotOS/pilot-os/src-tauri/src/render.rs:11-52, 56-72` | **Adapt.** Our `ColorTheme` slots: `modified`, `added`, `deleted`, `renamed`, `conflict`, `untracked`, `filename`, `dir_path`, `lines_added`, `lines_removed`, `hunk_header`, `section_header`. TTY-aware via `io::stdout().is_terminal()` (line 69). |
| Syntect-based syntax highlighting recipe (`SyntaxSet`, `ThemeSet`, `HighlightLines`, `as_24_bit_terminal_escaped`, `LinesWithEndings`) | `~/PilotOS/pilot-os/src-tauri/src/render.rs:14-19` and downstream code-block usage | **Keep verbatim.** Same imports, same call shape — applied to diff body lines (per-line highlight then prefix with `+`/`-`/` ` gutter). |
| `to_string.rs` header doc comment + module structure | `~/PilotOS/pilot-os/src-tauri/src/to_string.rs:1-13` (already self-cites the sessql pattern) | **Keep verbatim.** Header doc references back to sessql + lists every entry point that funnels through it. |
| `crossterm` `Stylize`/`SetForegroundColor`/`Print`/`ResetColor` idiom + `io::stdout().is_terminal()` for "should I emit ANSI?" decision | `~/PilotOS/pilot-os/src-tauri/src/render.rs:11-13, 9, 68-71` (Spinner::new sniffs `tty`), `:117-125` (`SetForegroundColor` → `Print` → `ResetColor`) | **Keep verbatim.** Same gating logic — `Format::Ansi` requested but stdout not a TTY → degrade to `Plain`. |
| MCP server skeleton: rmcp `tool_router` + `ServerHandler` + `Parameters<T>` + `schemars` schema derivation | `~/PilotOS/pilot-os/src-tauri/src/mcp_server.rs:11-50` and the rmcp imports in `~/sessql/crates/sessql-mcp/src/lib.rs:10-18` | **Combine.** PilotOS shows the in-process `tool_router` shape; sessql shows the stdio-bind shape. We use sessql's pattern (standalone stdio binary) since `gdl-mcp` is independent. |
| Banner (optional, suppressed by default — `--banner` opt-in for chat-like vibes) | `~/PilotOS/pilot-os/src-tauri/src/banner.rs:5-29` | **Optional.** Ship a small ASCII `gdl` banner; rendered only when `--banner` is passed. Off by default to keep output pipe-friendly. |

### 3.4 What is *new* in `gdl` (not copy-paste)

- The **rendered shape** itself — the section headers (`Staged Changes (N)`, `Changes (N)`, `Merge Changes (N)`), per-row layout (`badge  filename     dir_path     +N -N`), and the side-by-side hunk renderer.
- The mapping from `gix::status::Item` → our `GdlEntry` with first-class `ChangeSection` (§2.3).
- The **explicit `RenderOptions` contract** (§2.2): no ambient TTY/COLUMNS/NO_COLOR detection inside the shared helper. Both façades resolve into explicit options before calling.
- The **`--area` baseline selector for diff** (§2.4): explicit `worktree | staged | head`, not implicit.
- The **CLI ↔ MCP byte-equality tests** (Features 13a/13b) that prove both surfaces emit identical bytes for every meaningful `(format, color, width, view, area)` combination, not just the piped default.

### 3.5 Reuse policy — design reference, not source copy

Sibling source repos are AGPL-3.0-only (`~/sessql/Cargo.toml`, `~/kagmus/Cargo.toml`). `gdl` ships under MIT (see §10). The "Keep verbatim" wording elsewhere in this plan means **"reproduce the API shape, call pattern, and feature pinning verbatim from a design-reference reading of those files"**, NOT "paste source lines into gdl". Specifically:
- The `gix-diff` call sequence (`byte_lines` + `InternedInput` + `Algorithm::Histogram` + `diff_with_slider_heuristics`) is an idiomatic recipe published in the public `gix-diff` docs. Using it is not copying kagmus, and dependency versions are resolved through `cargo add`, not manually pinned.
- The `*_to_string` + MCP wrapper *pattern* is a structural design choice. Reimplemented from scratch in `gdl`, not lifted.
- `crossterm` and `syntect` idioms (`SetForegroundColor`, `HighlightLines`, `as_24_bit_terminal_escaped`) are documented usage from those crates' own docs, not PilotOS-specific code.

When this plan says "Keep verbatim shape", read it as "reproduce the structure independently".

---

## 4. Workspace layout

```
~/gdl
├── Cargo.toml                       # [workspace] members = ["crates/*"]
├── PLAN.md                          # this file
├── README.md                        # written in the final phase
├── LICENSE                          # MIT (see §10)
├── .gitignore                       # /target, *.swp, .DS_Store
└── crates/
    ├── gdl-core/                    # gix-based engine: open repo, status, hunks
    │   ├── Cargo.toml
    │   ├── src/lib.rs               # pub mod repo, status, hunks, types
    │   └── tests/
    │       ├── status_real_repo.rs
    │       ├── hunks_real_repo.rs
    │       ├── status_conflicted.rs
    │       ├── status_binary.rs
    │       └── rename_detection.rs
    ├── gdl-format/                  # *_to_string helpers (DRY surface)
    │   ├── Cargo.toml
    │   ├── src/lib.rs               # RenderOptions + status_to_string + diff_to_string
    │   └── tests/
    │       ├── format_text.rs
    │       ├── format_json.rs
    │       ├── format_ansi.rs       # strip_ansi(ansi) == plain + semantic-color oracle
    │       └── format_paths_only.rs
    ├── gdl-cli/                     # clap + crossterm renderer; produces `gdl` binary
    │   ├── Cargo.toml
    │   ├── src/
    │   │   ├── lib.rs               # pub fn run(cli) -> i32
    │   │   ├── cli.rs               # #[derive(Parser)] Cli + Subcommand
    │   │   ├── options.rs           # CLI args → RenderOptions resolution
    │   │   ├── main.rs              # 5-line: parse + run + exit
    │   │   └── banner.rs            # optional opt-in banner
    │   └── tests/
    │       ├── cli_status.rs        # assert_cmd; explicit --color/--width
    │       ├── cli_diff.rs          # assert_cmd; --area worktree|staged|head
    │       └── cli_errors.rs        # nonexistent repo → exit 1 + stderr text
    ├── gdl-mcp/                     # rmcp stdio wrapper; produces `gdl-mcp` binary
    │   ├── Cargo.toml
    │   ├── src/
    │   │   ├── lib.rs               # Server + tools (status, diff)
    │   │   └── main.rs              # 5-line: rmcp stdio bind
    │   └── tests/
    │       ├── mcp_stdio_status.rs           # real rmcp client/server round-trip
    │       ├── mcp_stdio_diff.rs             # area variants
    │       ├── mcp_errors.rs                 # nonexistent repo → error content
    │       ├── cli_mcp_byte_equality_status.rs  # ← invariant, status surface
    │       ├── cli_mcp_byte_equality_diff.rs    # ← invariant, diff surface
    │       └── cli_mcp_byte_equality_dogfood.rs # ← live ~/gdl, race-guarded
    └── gdl-testkit/                 # dev-only fixture builders (real gix repos)
        ├── Cargo.toml               # publish = false
        └── src/lib.rs               # pub fn make_status_fixture(...) -> (TempDir, Repository)
```

**Why 5 crates and not 1:** mirrors sessql's separation (`sessql-core` / `sessql-cli` / `sessql-mcp`) plus a shared **dev-only** `gdl-testkit` so fixture builders aren't duplicated across each crate's `tests/`. Integration-test modules in `crates/X/tests/` are not naturally shareable across crates; a `publish = false` dev-dep crate is the standard Rust workaround. Keeps `gdl-core` reusable as a Rust library and keeps the MCP rmcp dependency out of the CLI binary tree.

---

## 5. Phased feature plan (each phase = the full 7-step TDD loop)

Numbered features below are the **Red test** of each loop. After each feature's loop completes (`cargo test --workspace` clean, `cargo clippy` clean, `make release` clean), I mark it done in the session SQL todo table and proceed.

**Sequencing principle:** the CLI ↔ MCP byte-equality invariant (§2.2) is enforced **per surface, the moment that surface exists** — not deferred to a single end-of-plan test. Feature 13a runs immediately after Feature 12 (status tool) lands; Feature 13b runs immediately after Feature 12b (diff tool) lands. Contract drift is caught within one feature, not 7.

### Feature 1 — Workspace bootstrap with a real build proof
- **Red**: `crates/gdl-core/tests/smoke.rs` calls `gdl_core::version()` and asserts non-empty; fails RED because the function does not exist. (Not a string-grep — invokes a real function symbol from a real lib build.)
- **Green**: workspace `Cargo.toml`, repo-level `.cargo/config.toml` with `rustflags = ["-D", "warnings"]` (matching the sibling-repo pattern), root `Makefile` with `release`, five crate skeletons (`gdl-core`, `gdl-format`, `gdl-cli`, `gdl-mcp`, `gdl-testkit`), `pub fn version() -> &'static str { env!("CARGO_PKG_VERSION") }`.
- Proves: workspace builds, integration tests run, member crates wire together, `gdl-testkit` is consumed as a dev-dep by every other crate.

### Feature 1.5 — Private GitHub repo + initial commit pushed
- **Action** (post-bootstrap infra step, no test): after Feature 1's commit lands, run `gh repo create npiesco/gdl --private --source=. --remote=origin --description "Rich terminal git diff list — VSCode SC parity, DRY across CLI + MCP" --push`. Confirm via `gh repo view npiesco/gdl --json visibility,name` returning `"visibility":"PRIVATE"`.
- Prereq: `gh auth status` confirms `npiesco` is the active account (token scopes include `repo` and `delete_repo`).
- The repo stays private throughout development. Visibility flip is a future, conscious decision.

### Feature 2 — `gdl_core::open(path)` opens a real git repo
- **Red**: `crates/gdl-core/tests/open_real_repo.rs`:
  - `let tmp = tempfile::tempdir()?;`
  - `git init` via `std::process::Command` (real on-disk repo; no stub or source-grep)
  - `let repo = gdl_core::open(tmp.path())?;`
  - `assert_eq!(repo.worktree_dir(), tmp.path());`
  - `assert_eq!(repo.git_dir(), tmp.path().join(".git"));`
  - Asserts `gdl_core::open("/nonexistent")` returns the expected typed error variant (matched on, not stringly-compared).
  - Asserts `gdl_core::open(<subdir of repo>)` discovers upward to the repo root (`gix::discover` fallback).
- **Green**: thin wrapper over `gix::open` with `gix::discover` fallback, a small gdl-owned `Repository` wrapper exposing `git_dir()` / `worktree_dir()`, and a typed `OpenError`.

### Feature 3 — `gdl_core::status(&repo)` returns `Vec<GdlEntry>` with `ChangeSection`
- **Red**: `crates/gdl-core/tests/status_real_repo.rs`, plus `status_conflicted.rs`, `status_binary.rs`, `rename_detection.rs`. Each uses `gdl_testkit::make_status_fixture(...)` to build a real on-disk repo and exercises one cross-section of the §2.3 count rules:
  - **status_real_repo.rs** — commit `keep.txt`, then in the worktree: modify it, create `new.txt` (untracked), `git add` an `added.txt`, delete a tracked file, rename a tracked file. Assert the returned `Vec<GdlEntry>` (sorted per §2.3) contains exactly the expected `(section, kind, path, old_path, lines_added, lines_removed, is_binary)` tuples.
  - **status_conflicted.rs** — create two branches with conflicting edits to the same file, merge to force unmerged stages in the index, assert one `GdlEntry { section: Conflicted, kind: Conflicted, lines_added: 0, lines_removed: 0 }`.
  - **status_binary.rs** — commit a baseline binary blob (PNG bytes), modify it, assert `is_binary: true` with zero counts.
  - **rename_detection.rs** — `git mv`-style rename plus an edit; assert `kind: Renamed`, `old_path: Some(...)`, non-zero counts.
- **Green**: implement using `gix::status::Platform` for the worktree+index walk and a separate `gix::diff` HEAD-vs-index pass for staged entries. Translate each item to a `GdlEntry` with explicit `ChangeSection`. Detect binary via the gix-diff blob "is binary" check before invoking the hunk recipe. For text blobs, compute counts via the gix-diff hunk recipe: `byte_lines` + `InternedInput::new(old, new)` + `diff_with_slider_heuristics(Algorithm::Histogram, &input)` — see `~/kagmus/crates/ws-diff/src/hunks.rs:21-26` (verified citation). Sum `(after.end - after.start)` for `lines_added`, `(before.end - before.start)` for `lines_removed`. **Sort** before returning per §2.3.

### Feature 4 — `gdl_format::status_to_string` (Plain, no color, deterministic order)
- **Red**: `crates/gdl-format/tests/format_text.rs` — `let (tmp, repo) = gdl_testkit::make_status_fixture(...);` Call `gdl_format::status_to_string(&repo, RenderOptions { format: Plain, color: Never, width: 80, view: Full })`. Assert returned bytes equal a hand-written multi-line literal (including section headers `Staged Changes (N)` / `Changes (N)` / `Merge Changes (N)` / `Untracked (N)`, the badge column, filename, dir, `+N -N`). Test repeats with files created in randomized order to prove deterministic sort.
- **Green**: Single function `status_to_string(&Repository, &RenderOptions) -> Result<String, String>`. Builds sections by grouping entries by `ChangeSection`. The structural pattern is reproduced from sessql's `*_to_string` shape — see §3.5 for the reuse policy.

### Feature 5 — `status_to_string(format = Json)` for agent consumers
- **Red**: `crates/gdl-format/tests/format_json.rs` — same fixture, request `OutputFormat::Json`. Parse with `serde_json::from_str::<StatusOutput>`. Assert (a) the round-trip yields the exact `Vec<GdlEntry>` returned by `gdl_core::status`, and (b) the on-wire shape includes `section`, `kind`, `path`, `old_path`, `lines_added`, `lines_removed`, `is_binary` per entry, plus a top-level `version` field equal to `gdl_core::version()`.
- **Green**: `#[derive(Serialize, Deserialize)]` on `GdlEntry`, top-level `StatusOutput { version, entries }`, `serde_json::to_string_pretty`.

### Feature 6 — `status_to_string(format = Ansi, color = Always)` with a real semantic oracle
- **Red**: `crates/gdl-format/tests/format_ansi.rs`:
  - **6a — strip-ansi oracle**: call helper twice on identical input — once with `(format: Ansi, color: Always)`, once with `(format: Plain, color: Never)`. Assert `strip_ansi_escapes::strip(ansi_bytes) == plain_bytes`. This is the strong invariant: a stub that hardcodes one ANSI escape cannot satisfy it because the *content* must also match.
  - **6b — semantic color oracle**: for an entry with `kind: Modified`, locate the badge byte (`M`) in the ANSI output; assert the byte sequence immediately preceding it equals the theme's modified color escape (e.g. `\x1b[33m` if the theme picks yellow). For `Added`, the green escape. For `Deleted`, the red escape. **The expected escape is computed by calling `crossterm::style::SetForegroundColor(theme.modified).to_string()`** in the test, not hardcoded — so a theme change updates both sides.
  - **6c — no ambient detection**: confirm via an environment fixture that `NO_COLOR=1` set in the test process has **zero effect** on the helper output (the helper does not inspect env). Resolution lives in the CLI/MCP façade.
- **Green**: `ColorTheme` struct with named slots (`modified`, `added`, `deleted`, `renamed`, `conflict`, `untracked`, `filename`, `dir_path`, `lines_added`, `lines_removed`, `hunk_header`, `section_header`). Render with `crossterm::style::SetForegroundColor` / `ResetColor`. Helper ignores ambient state entirely — `color: ColorPolicy::Always` always emits escapes; `color: ColorPolicy::Never` never does.

### Feature 7 — `status_to_string(view = PathsOnly)`
- **Red**: `crates/gdl-format/tests/format_paths_only.rs` — fixture, request `(format: Plain, view: PathsOnly)`. Assert output is `\n`-joined entry paths in the §2.3 sort order, terminated by a single `\n`. Repeat with `(format: Json, view: PathsOnly)` — assert the JSON output is the same path list as a JSON array of strings (no entry metadata).
- **Green**: short-circuit in the helper when `view == PathsOnly`.

### Feature 8 — `gdl-cli` binary: `gdl status` + arg resolution to `RenderOptions`
- **Red**: `crates/gdl-cli/tests/cli_status.rs`:
  - Builds a fixture via `gdl_testkit`. Invokes `assert_cmd::Command::cargo_bin("gdl").args(["--repo", tmp, "--color", "always", "--width", "120", "status"])`. Captures stdout *bytes* (not string — bytes are the contract).
  - Independently calls `gdl_format::status_to_string(&repo, RenderOptions { format: Ansi, color: Always, width: 120, view: Full })`.
  - Asserts CLI stdout bytes == helper bytes **exactly** (no extra trailing newline). The CLI uses `print!`, never `println!`; the helper owns the trailing newline policy.
  - Repeat for `--color never` (expect Plain), `--paths-only` (expect PathsOnly), `--format json`.
- **Green**: clap struct with top-level `--repo <PATH>`, `--color <always|never|auto>`, `--width <N>`, `--paths-only`, `--format <ansi|plain|json>` and a `Status` subcommand (default when no subcommand given). `gdl_cli::options::resolve(args) -> RenderOptions` does the auto-resolution per §2.2. Dispatch via `run(cli) -> i32`. `main.rs` is 5 lines: parse, run, exit code.

### Feature 8.5 — `gdl` (no subcommand) defaults to `status` AND CLI error rendering
- **Red, part A**: `gdl --repo <tmp>` (no subcommand) ≡ `gdl --repo <tmp> status` byte-for-byte.
- **Red, part B**: `crates/gdl-cli/tests/cli_errors.rs` — `gdl --repo /definitely/nonexistent status` exits **1**, writes empty stdout, writes a stable error string to stderr (e.g. `gdl: cannot open repo at /definitely/nonexistent: ...`). Test pins the exact stderr prefix; the trailing cause may vary.
- **Green**: clap default-subcommand wiring; `gdl_cli::run` matches on the `OpenError` enum from `gdl_core::open` and formats per `Display`.

### Feature 9 — `gdl_core::diff(&repo, path, area)` returns real hunks
- **Red**: `crates/gdl-core/tests/hunks_real_repo.rs` — fixture file with three separate hunks, committed and then mutated. Call once per `area`:
  - `DiffArea::Worktree` — modify worktree only; assert hunks reflect worktree-vs-index delta.
  - `DiffArea::Staged` — `git add` the modifications; assert hunks reflect index-vs-HEAD delta.
  - `DiffArea::Head` — combined; assert hunks reflect worktree-vs-HEAD delta.
  Each assertion compares the returned `Vec<Hunk>` against expected `(old_start, old_lines, new_start, new_lines, old_bytes, new_bytes)` tuples. Bytes compared via raw `Vec<u8>` equality (CRLF/non-UTF-8 round-trip — gix-diff invariant from `~/kagmus/crates/ws-diff/src/hunks.rs:1-6`, verified citation).
- **Green**: resolve old blob bytes per `area` (HEAD vs index vs worktree), then run the gix-diff hunk recipe: `byte_lines(old).collect()`, `byte_lines(new).collect()`, `InternedInput::new(old, new)`, `diff_with_slider_heuristics(Algorithm::Histogram, &input)`, iterate `diff.hunks()`. Drop the SHA-256 digest fields that kagmus carries (we don't need hunk relocation).

### Feature 10 — `diff_to_string(format = Plain | Json, area)` with deterministic shape
- **Red**: `crates/gdl-format/tests/format_text.rs` (diff variant). Plain output is unified-diff shaped: `@@ -old_start,old_lines +new_start,new_lines @@` headers, `-`/`+`/` ` line gutters. Json output: `DiffOutput { version, file, area, hunks: Vec<HunkJson> }`. Assert both against literals. Binary file → Plain renders `Binary file <path> changed`, Json renders `{ "binary": true, "hunks": [] }`. Same `RenderOptions.width` is required and propagated, even though Plain ignores it for unified diff (Json embeds it for round-trip determinism).
- **Green**: dispatch on `format` inside the helper; the unified-diff renderer is straight string concatenation.

### Feature 11 — `diff_to_string(format = Ansi, color = Always, width)` with syntect oracle
- **Red**: `crates/gdl-format/tests/format_ansi.rs` (diff variant). Fixture is a Rust source file with edits that introduce a `fn` keyword, a string literal, and a `//` comment.
  - **11a — strip-ansi oracle**: `strip_ansi_escapes::strip(ansi) == plain` for the same `width` and `area`. (Same shape as Feature 6a.)
  - **11b — syntect ran**: in the test, independently load `SyntaxSet::load_defaults_newlines()`, find the Rust syntax, `HighlightLines::new(syntax, theme)`, highlight the new-side lines, render via `as_24_bit_terminal_escaped`. Locate the resulting keyword-color escape sequence in the test's independent output; assert that escape also appears in the helper's output, on a line matching the same source content. This proves syntect actually ran against the actual bytes for the actual language — not a hardcoded `\x1b[38;2;...m` slipped into a stub.
  - **11c — plaintext fallback**: same test, fixture file with extension `.txt` and no detectable syntax. Assert no syntect-style 24-bit escapes appear; gutter coloring (via `crossterm`) still applies.
  - **11d — width-driven layout**: same fixture, call once with `width: 200` (side-by-side, two columns) and once with `width: 60` (stacked unified). Assert the two outputs differ in shape *and* both satisfy `strip_ansi(...) == plain` against their respective Plain renderings at the same width.
- **Green**: cache `SyntaxSet` / `ThemeSet` in a `OnceLock` to keep large-diff rendering fast. `width >= 120` → two-column side-by-side; else stacked unified. Per-line: find syntax by file extension (`find_syntax_for_file(path)`), highlight new-side lines with syntect, gutter-prefix with crossterm. The width threshold (120) is documented in code and tested.

### Feature 12 — `gdl diff <path>` CLI subcommand
- **Red**: `crates/gdl-cli/tests/cli_diff.rs` — `gdl --repo <tmp> --color always --width 200 diff <path> --area worktree`; capture stdout bytes; compare byte-for-byte to `gdl_format::diff_to_string(&repo, path, RenderOptions { format: Ansi, color: Always, width: 200, view: Full }, DiffArea::Worktree)`. Repeat for `--area staged` and `--area head`.
- **Green**: dispatch `Diff { path, area }` subcommand; same `print!` discipline as Feature 8.

### Feature 13 — `gdl-mcp` stdio server: `status` and `diff` tools
- **Red**: `crates/gdl-mcp/tests/mcp_stdio_status.rs` spawns the `gdl-mcp` binary with `tokio::process::Command`, drives a real rmcp client over stdio (`rmcp::transport::TokioChildProcess` + `serve_client`), calls the `status` tool with explicit `{ repo_path, format: "Plain", color: "Never", width: 80, view: "Full" }`, asserts `CallToolResult.content[0].text == gdl_format::status_to_string(&repo, ...)`. Same shape for `mcp_stdio_diff.rs` against `diff` with each `area`.
- **Red, errors**: `crates/gdl-mcp/tests/mcp_errors.rs` calls `status` with `repo_path: "/nonexistent"`; asserts `CallToolResult.is_error == true` and the error content matches the same `Display` string the CLI prints to stderr (Feature 8.5b).
- **Green**: `lib.rs` with `#[tool_router]` ServerHandler, three tools (`status`, `diff`, `version`). Each tool body is the one-line wrapper: parse params → call into `gdl_format::*_to_string` → wrap `Ok`/`Err` as `tool_success`/`tool_error`. `main.rs` does `serve_server(stdio()).await?`. Tools have `Option<ColorPolicy> color` (default `Never`), `Option<u16> width` (default `80`), `Option<StatusView> view` (default `Full`), `Option<DiffArea> area` (default `Worktree`) — explicit, deterministic, never sniff env.

### Feature 13a — CLI ↔ MCP byte-equality (status surface, fixture + dogfood)
- **Red, fixture**: `crates/gdl-mcp/tests/cli_mcp_byte_equality_status.rs`. Build a fixture via `gdl_testkit`. For each `RenderOptions` row in a table — `{ (Plain, Never, 80, Full), (Ansi, Always, 120, Full), (Json, Never, 80, Full), (Plain, Never, 80, PathsOnly) }` — run the CLI via `assert_cmd` with the corresponding flags and the MCP `status` tool with the corresponding fields. Assert the two output byte streams are **equal** for every row.
- **Red, dogfood (race-guarded)**: same file, second test. Resolve `~/gdl` via `env!("CARGO_MANIFEST_DIR")`. Snapshot `git status --porcelain=v2 -z` of the live repo. Run CLI status. Run MCP status. Snapshot porcelain again. Assert (a) the two porcelain snapshots are identical (the repo did not change during the test), and (b) CLI stdout == MCP tool text byte-for-byte. If snapshot (a) fails, the test reports `repo changed during dogfood test — rerun` as a distinct failure mode, never a contract-equality failure.
- **Green**: nothing to implement if Features 8 and 13 are correct. If it fails, the contract drifted — fix the surface that diverged.

### Feature 13b — CLI ↔ MCP byte-equality (diff surface, fixture + dogfood)
- **Red, fixture**: `crates/gdl-mcp/tests/cli_mcp_byte_equality_diff.rs` — same shape as 13a, varying `(format, color, width, area)`.
- **Red, dogfood**: same race-guard pattern as 13a, diffing a stable file in `~/gdl` (e.g. `PLAN.md`) against each `area`.
- **Green**: same — contract validator, nothing to implement.

### Feature 14 — README + LICENSE + release builds + dogfood smoke
- README documents install (`cargo install --path crates/gdl-cli`), each subcommand, every flag with its resolution rule (§2.2), the `--repo` flag, the DRY claim with a `gdl-format` excerpt, MCP client config snippet (rmcp stdio), and a "use it on yourself" quickstart: `gdl --repo ~/gdl status`.
- `LICENSE` = MIT (see §10).
- `make release` produces `target/release/gdl` and `target/release/gdl-mcp`.
- Final dogfood smoke: `./target/release/gdl --repo $PWD status` runs against `~/gdl` itself, exits 0, non-empty output. Same for `./target/release/gdl --repo $PWD diff PLAN.md --area worktree`.
- Final `cargo test --workspace --release` (includes every fixture and every dogfood byte-equality test).

---

## 6. Workspace Cargo.toml dependency picks (locked at bootstrap)

Pins **verified against the actual sibling repos** (`~/sessql/Cargo.toml`, `~/kagmus/crates/ws-diff/Cargo.toml`, `~/PilotOS/pilot-os/src-tauri/Cargo.toml`) at planning time. After workspace bootstrap, run `cargo tree -d` to confirm no duplicated incompatible `gix-*` components — adjust `gix` minor if collisions appear.

| Dep | Version | Verified source / justification |
|---|---|---|
| `gix` | Cargo-resolved via `cargo add gix --package gdl-core` | Workspace MSRV is `rust-version = 1.91`, matching the installed stable toolchain. Cargo currently resolves `0.84.0`. kagmus uses individual `gix-*` component crates rather than a top-level `gix`; gdl wants the umbrella crate for `open`/`status`. |
| `gix-diff` | Cargo-resolved via `cargo add gix-diff --package gdl-core` | Use the same public API recipe as kagmus, but do not request a version in the `cargo add` command. Workspace MSRV is `rust-version = 1.91`; Cargo currently resolves `0.64.0`. |
| `crossterm` | `0.29.0` | Verified `~/PilotOS/pilot-os/src-tauri/Cargo.toml:40`. |
| `syntect` | `5.3.0` | Verified `~/PilotOS/pilot-os/src-tauri/Cargo.toml:42`. Provides `as_24_bit_terminal_escaped`. |
| `strip-ansi-escapes` | latest | Required for Feature 6a/11a oracle. |
| `clap` | `4` features = `["derive"]` | PilotOS pattern. |
| `serde`, `serde_json` | latest | JSON output. |
| `schemars` | `1` | Verified `~/sessql/Cargo.toml:21`. (Earlier draft said `0.8`; corrected.) |
| `rmcp` | `1.6.0` features = `["server", "transport-io", "macros"]` | Verified resolved version from `~/sessql/Cargo.lock` (`Cargo.toml` declares `1.3.0`, Cargo currently resolves `1.6.0`). Earlier draft said `0.1`; corrected. |
| `tokio` | `1` features = `["rt-multi-thread", "macros", "process", "io-util"]` | rmcp + child-process MCP tests. |
| `tempfile` | latest | Real-repo fixtures in `tests/`. |
| `assert_cmd` | latest | Real binary invocation in CLI tests. |
| `predicates` | latest | Assertion helpers for assert_cmd. |
| `tracing`, `tracing-subscriber` | latest | Structured logging for debug iteration (step 4 of the TDD loop when something fails). |
| **Banned** | — | No `pretty_assertions` substring-only macros, no `mockall`, no `mockito`, no `mock_*` of any kind, no `pretty_assertions` for unstructured matching. |

---

## 7. Validation checklist (run at the close of every feature loop)

- [ ] `cargo test --workspace` — all integration tests green
- [ ] `cargo fmt --all -- --check`
- [ ] `cargo clippy --workspace --all-targets -- -D warnings`
- [ ] `make release`
- [ ] No `#[ignore]`, `todo!()`, `unimplemented!()`, `mock`, `stub`, `fake`, `"for now"`, `"later"`, `"defer"` in the touched files (`rg` audit before declaring done)
- [ ] Every new test calls real public API against real I/O — no `fs::read_to_string` of a `.rs` file with `.contains(...)` assertions
- [ ] Any new `*_to_string` helper takes its full `RenderOptions` explicitly — helper does not read env, TTY, or `COLUMNS`
- [ ] Any new CLI subcommand uses `print!` (not `println!`); helper owns the final newline
- [ ] When a new tool/flag pair is added on either CLI or MCP, the byte-equality table in Feature 13a/13b grows a row covering it (or a deferral is filed)
- [ ] SQL todo row marked `done`

---

## 8. Out of scope (explicit)

To keep the surface tight and the DRY contract enforceable:

- **No staging / committing / writes** of any kind. Read-only diff renderer.
- **No TUI** — no alt-screen, no key handling, no panes. Single `print!` output.
- **No REPL** — explicitly *not* part of `gdl`; the user's requirement was "without the repl".
- **No git operations beyond `status` + `diff`** — no `log`, no `blame`, no `branch`.
- **No remote operations** — no fetch, no auth, no SSH.
- **No multi-repo / submodule recursion in v0.1** — top-level only.
- **No user-configurable theme in v0.1.** Single fixed `ColorTheme` shipped in `gdl-format`. `GDL_THEME=<name>` and config files are deferred to v0.2 with explicit issues filed at release.
- **No output truncation in v0.1.** Large diffs render fully; `OnceLock`-cached `SyntaxSet`/`ThemeSet` keep cost down. A `--max-lines` flag is deferred to v0.2.
- **No performance budget enforced as a test in v0.1** beyond "the dogfood smoke runs end-to-end in CI". Profiling is a v0.2 concern.

---

## 9. Session-state mirror

This plan is the source of truth at `~/gdl/PLAN.md`. A mirror lives at the current session's `~/.copilot/session-state/<session_id>/plan.md` for tooling that reads from there.

The SQL `todos` table tracks per-feature execution status (`pending` → `in_progress` → `done`) and depends-on edges encode the TDD ordering. Feature 13a/13b "ready" depends on Feature 8 + Feature 13 having status `done`.

---

## 10. License and reuse policy

**License:** MIT, in `LICENSE` at repo root. Chosen so `gdl` is freely reusable as a CLI binary or as a Rust library by other tools (including those that don't want AGPL).

**Reuse policy (clarifies "Keep verbatim" wording elsewhere in this plan):** sibling repos are AGPL-3.0-only (`~/sessql/Cargo.toml:14`, `~/kagmus/Cargo.toml:8`). We do NOT copy AGPL source into the MIT-licensed gdl tree. What we DO reuse:

1. **Public-API call patterns** of third-party crates — e.g. the `gix-diff` recipe (`byte_lines` + `InternedInput` + `Algorithm::Histogram` + `diff_with_slider_heuristics`), `crossterm`'s `SetForegroundColor`, `syntect`'s `HighlightLines` + `as_24_bit_terminal_escaped`. These are documented in those crates' own docs; reading kagmus or PilotOS is faster than rediscovering them, but we're using the crate APIs, not copying repo code.
2. **Structural design patterns** — e.g. sessql's "one `*_to_string` helper per operation, MCP tool body is a one-line wrapper" pattern. The pattern itself is a design choice, not copyrightable. We reimplement it in our own crate structure.
3. **Version pins** for shared dependencies — picking the same `gix-diff` minor as kagmus avoids gix-component duplication; that's an integration choice, not a copy.

When this plan says "Keep verbatim", read it as **"reproduce the structure independently to match the pin/API shape"**. No source files are copied across repos.

If `gdl` ever needs to lift actual code (e.g. a non-trivial parsing helper) from a sibling AGPL repo, this plan must be updated first to either (a) relicense gdl, or (b) carve that helper into a separately-licensed dep.
