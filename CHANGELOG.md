# Changelog

All notable changes to this project will be documented in this file.
Format follows [Keep a Changelog](https://keepachangelog.com/en/1.0.0/).

---

## [0.2.0] — 2026-06-21 — REST API (in progress)

### Added
- `terapi run <campaign.toml> --silent` (`-s`) — suppresses all output, returns exit code 0/1 for CI/cron use

### Changed
- Version bump to 0.2.0, beginning of interactive REST API implementation
- Author updated to Thierry Soulie <thierry.soulie@tsodev.fr>

---

## [0.1.0] — 2026-06-21 — Foundation

### Added

**TUI skeleton**
- 3-tab layout: Request / Collections / History
- Tab switching via `Tab` key
- Quit via `q` or `Esc`
- Status bar with contextual keybinding hints

**Request panel**
- Sub-tabs: Description / Headers / URL Params / Body / Auth / Options
- `←` / `→` to navigate sub-tabs

**Collections panel**
- Foldable folder/subfolder tree with `▶` / `▼` icons
- `↑` / `↓` to navigate, `Enter` to expand/collapse

**Response viewer**
- 3-column JSON table: Key / Type / Value with depth indentation
- Foldable nodes (`Enter` to fold/unfold, `▶` / `▼` icons)
- Inline content preview for folded objects and arrays
- `-` / `=` to resize the Key column (AZERTY-friendly)
- `r` to toggle between JSON view and Raw text view
- `↑` / `↓` to navigate rows (JSON) or scroll (Raw)

**JSON highlight module** (`src/json_highlight.rs`)
- Recursive `serde_json::Value` walker producing a flat `Vec<JsonRow>`
- Fold state via JSON Pointer paths in a `HashSet<String>`
- Inline previews for folded objects (`{ street: "12 rue…", city: "Paris" }`)
  and arrays (`[ "rust", "tui", "graphql" ]`)

**CLI** (`clap` 4)
- `terapi` — launches TUI
- `terapi --demo <file>` — loads a JSON file into the response viewer
- `terapi --version` / `terapi --help`

**Campaign runner** (`terapi run <campaign.toml>`)
- Headless mode — no TUI, runs from the terminal
- TOML campaign format: `[campaign]`, `[env]`, `[[steps]]`
- Variable substitution `{{VAR}}` in url / headers / body
- Variable extraction from JSON responses via dot-path (`token`, `user.id`)
  — extracted values injected into subsequent steps
- Real HTTP execution via `reqwest` (GET / POST / PUT / PATCH / DELETE)
- CSV connector: `[[connectors]] type = "csv" path = "..."` runs the
  campaign once per row, CSV columns become `{{variables}}`
- Campaign report: live step-by-step progress + boxed summary with
  timing, status codes, extracted values, and failure details

**Examples**
- `demo.json` — realistic nested API response for TUI demo
- `examples/users.toml` — campaign with login → JWT extraction → CRUD steps
- `examples/bulk_invite.toml` — data-driven campaign with CSV connector
- `examples/contacts.csv` — sample contact list for bulk_invite
