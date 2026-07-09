# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What this is

`terapi` is a single-binary Rust TUI (ratatui + crossterm) for exploring and automating REST/GraphQL APIs, plus a headless campaign runner and a TUI campaign builder — all in one `terapi` executable with three entry points via `clap` subcommands in `src/main.rs`:

- `terapi` (no subcommand) — launches the interactive TUI (`src/ui.rs` + `src/app/`)
- `terapi run <campaign.toml>` — headless campaign runner (`src/campaign.rs`)
- `terapi build [campaign.toml]` — TUI campaign editor (`src/builder/`)
- `terapi import <file>` — Postman/Insomnia/terapi-TOML importer (`src/import/`)

Not a Cargo workspace — one crate, one binary (`Cargo.toml` has no `[lib]`, only `[[bin]] name = "terapi"`).

## Commands

```bash
cargo build --release      # release binary → target/release/terapi
cargo check                 # fast type-check, no codegen (preferred while iterating)
cargo run -- --demo demo.json     # launch TUI with a JSON file pre-loaded
cargo run -- run examples/campaigns/crud_demo.toml   # headless runner
cargo run -- build          # campaign builder
```

**There are no automated tests in this repo** (`grep -rn '#\[test\]' src` returns nothing, no `tests/` directory). There is also no `.github/` CI, no `.cursorrules`/`.cursor/`, and no `copilot-instructions.md` — this file is the only project-level guidance that exists. Do not assume a test suite exists; verify behavior by running the binary directly against `examples/campaigns/*.toml` (no API key required for most of them — see the table in README.md) or against the TUI.

No `clippy.toml`/`rustfmt.toml` — defaults apply if you run `cargo clippy` / `cargo fmt`.

`Cargo.lock` is gitignored (this is a bin crate published to crates.io) — don't expect it to be committed, and don't rely on it being present for reproducibility across sessions.

### Known dependency trap (from CHANGELOG 0.10.4)

`Cargo.toml` pins `time = ">=0.3, <0.3.52"` with a comment explaining why: `time 0.3.52` changed `Parsable::parse()`'s signature in a semver-compatible patch release, which broke `cookie 0.18.1` (pulled in transitively via `reqwest`'s cookie support) and made `cargo install terapi` fail to build entirely. If a future `cargo update` reintroduces a build break in this area, check whether the pin needs adjusting rather than assuming the code is at fault.

## Architecture

### Layered structure

```
main.rs           CLI parsing (clap), dispatches to one of the three modes, owns the
                   panic hook (restores terminal on panic — see below) and the pending-*
                   side-channel handling for external editor/diff tools.
app/               TUI application state + logic for the main `terapi` binary
  mod.rs             App struct (~30 fields groups: collections, environments, request
                     builder, auth, response viewer, history, GraphQL, schema
                     introspection, campaigns tab, OAuth2) + handle_key()/tick()
  request.rs, response.rs, collections.rs, envs.rs, campaigns_tab.rs, oauth2.rs,
  schema.rs, http.rs, gql_completion.rs, var_picker.rs, types.rs
                     one file per panel/concern; all `impl App` blocks, split out of
                     mod.rs to keep it navigable
ui.rs (3.4k lines) All rendering — pure `fn(&Frame, &App, Rect)` functions, one per
                   panel/sub-tab/modal. No state mutation happens here.
builder/           `terapi build` — a second, independent TUI (own event loop, own
                     BuilderApp struct) for editing campaign TOML interactively
  mod.rs             BuilderApp struct + run()/run_builder() event loop
  ui.rs, step_editor.rs, editor.rs, browser.rs, checker.rs, types.rs
campaign.rs (2.5k) The core campaign *engine* — TOML schema (Campaign/Step/Assertion/
                   Transform/...), variable resolution, and the step executors
                   (run_single_step, run_loop_step, run_poll_step, run_jq_step,
                   run_parallel_step, run_transform_step, run_search_step, ...).
                   Used by both `terapi run` (CLI) and the TUI's Campaigns tab/builder
                   step preview.
connector.rs       Input connectors (CSV / JSON file / JSON-from-seed-step) that turn
                   external data into iteration rows for a campaign.
storage.rs         Everything about the on-disk terapi directory: resolving
                   TERAPI_DIR, loading/saving collections, envs, history, campaigns;
                   also built-in {{VAR}} resolution (resolve_builtin_vars) and
                   {{VAR}} substitution from an env map (resolve_vars).
import/            Postman v2.1 and Insomnia v4 importers → terapi TOML.
xml_convert.rs     Converts XML/HTML response bodies to a JSON-shaped tree so the
                   same JSON viewer/extract/diff code paths work for XML responses.
json_highlight.rs  Flattens/tokenizes JSON into rows for the windowed table-based
                   JSON tree viewer (perf-critical — see CHANGELOG 0.10.9/0.10.10).
event.rs           Thin crossterm event/tick loop (250ms tick), shared idea used by
                   both the main TUI and the builder TUI.
```

### The campaign engine is the shared core

`campaign.rs::run_streaming()` is the single execution engine: it walks `Campaign.steps`, resolves `{{VAR}}` at each step, executes the step per its `kind` (`http`/`graphql`/`transform`/`seed`/`loop`/`poll`/`search`/`set`/`jq`/`parallel`/`notify`/`build`/`file`/`pause`/`comment`), and emits `CampaignEvent`s over an `mpsc::UnboundedSender`. Three different frontends consume that same stream:

1. `campaign::run()` — the CLI entry point (`terapi run`), spawns `run_streaming` and prints/collects results (text/json/csv via `OutputFormat`).
2. `app/campaigns_tab.rs` — TUI Campaigns panel, drives it via `App::campaign_rx`/`campaign_tx` and renders live per-step results as they arrive.
3. `builder::run_step_preview_with_context()` (also in `campaign.rs`) — the builder's single-step "run" preview (`r` key), reusing the same executors so a step behaves identically whether previewed in the builder or run for real.

This is why step-execution bug fixes in `campaign.rs` affect all three surfaces at once, and why new step `kind`s need to be wired into: the `Step` struct fields, the `kind` match in `execute_step`/`run_single_step`, the builder's brick catalog + `step_editor.rs` + `checker.rs` (static validation), and the badge/label tables in both `ui.rs` (TUI campaigns tab) and `builder/ui.rs` (pipeline view).

### Variable resolution model

Variables (`{{VAR}}`) are plain string substitution over `HashMap<String, String>` env maps, layered by priority (documented in README): built-ins (`storage::resolve_builtin_vars` — `{{DATE}}`, `{{UUID}}`, `{{TIMESTAMP}}`, etc., support `±N` arithmetic) → `env_file` → `[env]` → `[[params]]` defaults → connector row → step `env` → extracted vars → runtime `-p` overrides. `campaign.rs::resolve()`/`resolve_value()` apply substitution to strings/JSON values; `extract_at()`/`extract_value_at()`/`extract_segments()` implement the dot-path extraction language (including `*` wildcard over arrays) used by both `[steps.extract]` and `foreach`.

### Storage / config resolution

`storage::resolve_terapi_dir()` picks the data directory in priority order: `$TERAPI_DIR` env var → `./.terapi/` (if it exists — per-project, git-friendly) → `~/.config/terapi/` (global default, via `dirs::config_dir()`). Collections, environments, history, and campaigns are all TOML files under this directory (`collections/`, `envs/`, `campaigns/`), loaded via `storage::load_collections()`/`load_envs()`/`load_campaigns()`/`load_history()`.

### Async pattern

Single `#[tokio::main]` in `main.rs`. The TUI itself is a synchronous render/poll loop (`event::poll` with a 250ms tick in `event.rs`) — async work (HTTP sends, schema introspection, campaign runs, OAuth2 token fetch) is spawned with `tokio::spawn` and results come back through per-concern `mpsc::UnboundedReceiver`s stored on `App`/`BuilderApp` (`response_rx`, `schema_rx`, `campaign_rx`, `oauth2_rx`, `step_preview_rx`) and drained each frame in the main loop — this keeps rendering non-blocking while requests are in flight. `reqwest::Client` instances are built once and reused (not per-request) so the cookie jar persists; note `.pool_max_idle_per_host(0)` is set on all 4 client-builder call sites (CHANGELOG 0.10.10) to avoid a dead-keepalive-connection hang after a large transfer — don't remove it without re-reading that changelog entry.

### External tool integration

The TUI shells out to external tools for a few things, each gated by an env var with a fallback, all wired through `main.rs`'s `run_tui()` pending-flag pattern (`app.pending_*` booleans/options set by `app/`, drained after each frame): `$TERAPI_JSON_EDITOR` (body/response editing, default `jsoned`), `$TERAPI_JSON_DIFFER` (structural diff, takes priority) or `$TERAPI_DIFF` (fallback to `diff -u | less`), and `$EDITOR`/`$VISUAL` (opening a collection/campaign TOML directly, default `vi`). `terapi-env.sh` at the repo root auto-detects `jsoned`/`difft`/`delta` on `PATH` and exports these before exec'ing `terapi`.

### Panic safety

`main.rs::install_panic_hook()` wraps the default panic hook to force `disable_raw_mode()` + `LeaveAlternateScreen` before the panic message prints — without it, a panic while either TUI (main or `terapi build`) is in raw/alternate-screen mode leaves the user's terminal broken until `reset`/`stty sane`. Keep this in place if refactoring `main()`.

## Examples directory

`examples/campaigns/*.toml` and `examples/collections/*.toml` are runnable references, not just docs — README.md documents which ones need external credentials (`horaires_sncf_par_gare.toml` needs a terapi env named `sncf` with `SNCF_TOKEN`; `crates-io-updates-last-hour.toml` needs `jq` on `PATH`). When adding a new step `kind` or campaign feature, adding/extending one of these examples is the de facto way this repo demonstrates and manually verifies new behavior (there being no test suite).

## Documentation files

- `README.md` — user-facing overview, keybindings, campaign TOML reference, stack table.
- `USAGE.md` (165KB) — the exhaustive reference manual; go here for details README only summarizes.
- `CHANGELOG.md` — Keep a Changelog format; recent entries often explain *why* a piece of code looks the way it does (perf fixes with benchmarks, TTY-inheritance fixes, dependency pins) — worth checking before "simplifying" something that looks odd.
- `terapi-keymap.html` / `.pdf` — printable keybinding cheat sheets, generated artifacts, not source of truth (README's keybinding tables are).
