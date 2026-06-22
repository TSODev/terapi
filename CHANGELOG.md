# Changelog

All notable changes to this project will be documented in this file.
Format follows [Keep a Changelog](https://keepachangelog.com/en/1.0.0/).

---

## [Unreleased]

### Added
- **Active env indicator in Request panel**: the URL bar title now shows ` · env: <name>` when an environment is active, making the active environment visible while building requests
- **`env_file` in campaign TOML** — reference a named terapi environment as the base variable set:
  ```toml
  env_file = "production"   # loads <terapi_dir>/envs/production.toml
  ```
  Inline `[env]` vars take precedence over `env_file` vars
- **Per-step `env` field** — each step can point to a named terapi environment to use for that step:
  ```toml
  [[steps]]
  name = "Health check (staging)"
  env  = "staging"   # loads staging env vars, overrides campaign base for this step only
  method = "GET"
  url    = "{{BASE_URL}}/health"
  ```
  Extracted vars from previous steps still override the step env (highest priority)
- `storage::load_env_by_name(name)` — load a single terapi environment by name (used by campaign runner)
- `storage::resolve_vars(text, vars)` — `{{VAR}}` substitution helper (foundation for TUI request sending)
- **Send request** — interactive HTTP requests from the TUI Request panel:
  - `e` — enter URL edit mode (URL bar highlighted, cursor visible)
  - `←` / `→` in URL mode — cycle HTTP method (GET / POST / PUT / PATCH / DELETE)
  - `Enter` — send request and return to response mode
  - `Esc` — exit URL edit mode without sending
  - `s` — send the current request from response mode (without re-entering edit)
  - `m` — cycle method in response mode
  - Async execution via `tokio::spawn` + `mpsc::unbounded_channel`; result polled in `tick()`
  - `{{VAR}}` placeholders in the URL resolved from the active environment before sending
  - Response block title shows status code (color-coded) + elapsed time (ms)
  - Loading indicator `⟳ sending…` while request is in flight
- Example collections for the TUI (`examples/collections/`):
  - `public-rest.toml` — JSONPlaceholder, ReqRes, httpbin, PokeAPI, CoinGecko (5 folders)
  - `graphql.toml` — Countries API and Rick & Morty API (GraphQL via POST, ready for v0.5)
- Example campaigns:
  - `examples/crud_demo.toml` — full CRUD on JSONPlaceholder (POST → extract id → GET → PUT → PATCH → DELETE)
  - `examples/auth_flow.toml` — ReqRes auth flow (login → extract token → GET user → PUT update)
  - `examples/debug_toolbox.toml` — httpbin.io edge cases (status codes, headers, bearer auth)
- **URL Params editor** — key/value list in the URL Params sub-tab:
  - `a` — add a param (Key + Value modal, `Tab` to switch fields)
  - `d` — delete selected param
  - `Enter` — edit selected param
  - `↑` / `↓` — navigate params
  - Params are appended to the URL as query string at send time (`?k=v&k2=v2`)
  - Loading a collection request with a query string splits it automatically: base URL goes to the URL bar, params populate the list
- **Load request from Collections** — press `Enter` on a request node to load it into the Request tab:
  - Method, URL, headers, and body are all restored
  - App switches automatically to the Request tab
  - Response area is cleared; status bar shows the loaded request name
  - Folders still expand/collapse as before
- **Body editor — dual mode** (Text + JSON key/value):
  - `t` — toggle between Text and JSON modes (when Body sub-tab is active, outside edit mode)
  - Switching syncs content: Text → JSON parses the textarea as a JSON object and populates the field list; JSON → Text serializes pairs back to pretty-printed JSON in the textarea
  - **Text mode** (`tui-textarea`): `i` to enter, full multi-line editing, `Esc` to exit
  - **JSON mode** (key/value table): `i` to enter the field editor, then `a` add, `d` delete, `Enter`/`e` edit selected, `↑`/`↓` navigate, `Esc` exit
  - On send: JSON mode auto-types values — integers, floats, `true`/`false`, `null` are inferred; anything else serialized as a JSON string
  - Empty body (no text / no fields) sends no request body
  - `tui-textarea = "0.7"` added as dependency

### Changed
- Collection CRUD in the TUI (Collections panel):
  - `n` — create a new collection (name input modal)
  - `f` — create a new folder inside the selected collection (name input modal); cursor moves to the new folder automatically so `a` can be pressed immediately
  - `a` — add a request to the selected collection or folder (name / method / URL modal, `Tab` cycles fields, `←`/`→` cycles HTTP method)
  - `d` — delete the selected item (collection, folder, or request) with a confirmation modal
- Workflow `n f f a` now works as expected: each `f` lands the cursor on the newly created folder, so `a` inserts into that folder directly
- Modal overlay system: `NewCollection`, `NewRequest`, `ConfirmDelete` — centered, drawn over existing UI with `ratatui::widgets::Clear`
- `delete_collection()` in `storage.rs`
- `StoredRequest::new()` constructor
- `src/storage.rs` — TOML-based local storage for collections
  - `resolve_terapi_dir()` — priority resolution: `TERAPI_DIR` env var → `./.terapi/` (project-local) → `~/.config/terapi/` (XDG global fallback)
  - `load_collections()` — reads all `.toml` files from `<dir>/collections/` at startup
  - `save_collection()` — serialises a collection to a named TOML file; called on every mutation
  - Collection schema: `[collection]`, `[[folders]]`, `[[folders.requests]]`, `[[requests]]`
- `examples/collection.toml` — annotated template documenting the collection TOML format
- `dirs` crate dependency for cross-platform config directory resolution
- Empty Collections panel now shows a hint: "No collections — press n to create one"
- New **Env** top-level tab (Request | Collections | **Env** | History):
  - Two-panel layout: environment list (left, 30%) | key=value variables (right, 70%)
  - `●` indicator on the active environment
  - `n` — create a new environment
  - `a` — add a variable to the selected environment (Key + Value modal, `Tab` cycles fields)
  - `d` — delete the selected environment or variable (depends on focus)
  - `Enter` — activate the selected environment (focus on env list)
  - `←` / `→` — switch focus between the two panels
  - Variables displayed sorted alphabetically
- New storage functions: `load_envs`, `save_env`, `delete_env` — one TOML file per env in `<terapi_dir>/envs/`

### Changed
- `App` state: `Vec<CollectionNode>` replaced by `Vec<StoredCollection>` (source of truth) + `HashSet<String>` for expand/collapse state (`"c0"`, `"c0f1"`, …)
- `flatten_stored()` replaces `flatten_collections()` — produces context-aware `FlatNode` with `NodeAddress` for direct indexing into `stored_collections`
- `App::new()` loads collections from disk at startup; falls back to built-in sample collections when no files are found

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
