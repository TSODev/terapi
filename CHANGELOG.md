# Changelog

All notable changes to this project will be documented in this file.
Format follows [Keep a Changelog](https://keepachangelog.com/en/1.0.0/).

---

## [Unreleased]

---

## [0.6.7] — 2026-06-24 — Fix panic UTF-8 dans le rendu campaigns

### Fixed
- **Panic sur noms de steps non-ASCII** — `render_step_result_line()` tronquait le nom du step par index d'octet (`&s[..21]`), ce qui provoquait un panic si un caractère multi-octet (ex. `é`) chevauchait la frontière. Corrigé avec `chars().count()` / `chars().take()`. Même correction appliquée aux valeurs de variables extraites et aux labels de colonnes CSV dans `render_campaigns_panel()`.

---

## [0.6.6] — 2026-06-23 — Campaign parameters & external editor

### Added
- **Campaign parameters** (`[[params]]`) — declare user-facing inputs in the campaign TOML with `name`, `description`, and `default`. Internal variables stay in `[env]`; params are intended to be overridden at run time.

  ```toml
  [[params]]
  name        = "DEPART"
  description = "Ville de départ"
  default     = "Paris"

  [[params]]
  name        = "ARRIVEE"
  description = "Ville d'arrivée"
  default     = "Lyon"
  ```

  Variable priority: `env_file` → `[env]` → param defaults → runtime overrides (CLI or TUI).

- **CLI param overrides** — `-p` / `--param KEY=VALUE` (repeatable) on `terapi run` overrides any param:

  ```bash
  terapi run itineraire_demo.toml -p DEPART=Bordeaux -p ARRIVEE=Nantes
  ```

  The CLI header now lists all params and their effective values before running.

- **TUI params modal** — pressing `r` on a campaign with `[[params]]` opens a form modal instead of starting immediately. Values are pre-filled from the defaults. `↑`/`↓` navigates, `Enter` edits the selected value, `r` launches the campaign with the current values, `Esc` cancels without running.

- **Itinerary campaign example** (`examples/itineraire_demo.toml`) — demonstrates the full params + pipeline flow: geocode two French cities via the IGN Géoplateforme API, compose coordinates with a transform step, then compute the road itinerary (distance + duration). No API key required. `DEPART`, `ARRIVEE`, `PROFILE`, and `OPTIMIZATION` are declared as `[[params]]` so each run can target different cities.

- **Open in external editor (`E`)** — pressing `E` on a selected item in the Collections or Campaigns tab suspends the TUI, launches `$EDITOR` (fallback: `$VISUAL`, then `vi`) with the corresponding TOML file, and waits for the editor to close. On return, both collections and campaigns are reloaded from disk automatically. Works with any terminal editor (vim, neovim, nano, helix…) or GUI editors that block the terminal (e.g. `EDITOR=code --wait`).

---

## [0.6.5] — 2026-06-23 — Connector pipeline & UX improvements

### Added
- **GraphQL query autocompletion** — `Ctrl+Space` in the query editor (insert mode) opens a magenta completion popup. If a type detail is loaded from the Schema tab, its fields are proposed (name + type). Otherwise, all OBJECT/INTERFACE/INPUT_OBJECT type names are listed. Typing filters in real time; `Enter` or `Tab` inserts the selection (replacing the prefix already typed); `Esc` closes.
- **History — GraphQL entries** — History now records `graphql`, `graphql_query`, and `graphql_variables` for every request. GraphQL entries show a magenta `GQL` badge in the list instead of the HTTP method. Loading a GraphQL entry (`Enter`) activates GraphQL mode, restores the query and variables, and positions the Request tab on the Query sub-tab. REST entries behave as before. Existing `history.toml` files remain valid (`#[serde(default)]`).
- **JSON extraction path bar** — a permanent line below the JSON response table shows the dot-notation path of the currently selected row (e.g. `↳ features.0.properties.city`). The path matches the format expected by `[steps.extract]` in campaigns — navigate to any key with `↑`/`↓` and copy the path directly into your TOML.
- **JSON response search** — press `/` in the JSON view to open a search bar. Type to filter rows by key or value (case-insensitive); matching rows are highlighted in yellow and bold; the cursor jumps to the first match automatically. `>` navigates to the next match (wraps), `<` to the previous. `Esc` closes the bar and clears the filter.
- **URL params auto-parse from URL bar** — pasting a full URL with a query string (e.g. `https://api.example.com/search?q=foo&limit=10`) into the URL bar and pressing `Esc` or `Enter` now automatically splits it: base URL stays in the URL bar, query parameters populate the URL Params tab. Same parsing applies when loading a request from History.
- **URL bar reconstructs full URL** — in read mode (outside URL edit mode) the URL bar displays `base?key=val&key2=val2` so the full effective URL is always visible; edit mode shows only the base URL for clean editing.
- **History deduplication** — sending a request identical to an existing history entry (same method + URL + body, or same URL + query for GraphQL) moves the existing entry to the top instead of creating a duplicate.
- **JSON connector** (`type = "json"`) — new campaign connector type that iterates over a JSON array. `path` points to a local JSON file; `select` (optional dot-path) navigates to the target array inside the file (omit or set to `""` for root). Object fields are flattened with dot-notation; nested arrays serialised as JSON strings. See `examples/json_connector_demo.toml`.
- **Seed step** (`kind = "seed"`) — a campaign step that runs once before the iteration loop and whose JSON response body feeds the `[[connectors]]` block via `from_step = "step name"`. Enables fully HTTP-driven data-driven campaigns without a local file. The seed step is skipped in the iteration loop. See `examples/seed_step_demo.toml`.
- **Output connector** (`[[outputs]]`) — after all iterations complete, writes a JSON array of step response bodies to disk. Fields: `from_step` (step name to collect), `path` (destination file), `select` (optional dot-path into each response body). Failed iterations are skipped. Parent directories created if needed. Multiple `[[outputs]]` blocks supported per campaign. CLI confirms each written file at the end of the report.
- **New campaign examples** — `examples/json_connector_demo.toml` (JSON file connector, JSONPlaceholder), `examples/seed_step_demo.toml` (seed step + output connector, French geo API), `examples/users.json` (sample data).

- **Pause step** (`kind = "pause"`) — inserts a deliberate wait between steps without making an HTTP request. `wait_ms` sets the delay in milliseconds. Appears as `WAIT` in CLI output and TUI. Useful for rate-limiting: avoid being throttled by APIs that cap requests per second.

  ```toml
  [[steps]]
  name    = "Rate limit pause"
  kind    = "pause"
  wait_ms = 1000   # wait 1 second before the next step
  ```

### Fixed
- **Request tab status hints** — switching to the Request tab via `Tab` now shows the full context-aware hint for the active sub-tab instead of the generic `Tab: switch panel ←/→: section q: quit`. Each sub-tab now exposes its key actions: `e`, `m`, `g`, `n`, `i`, `a`, `d`, `s`, `S` as appropriate.
- **Stale URL params when loading GraphQL from History** — loading a GraphQL History entry now resets the URL params list before parsing, preventing parameters from a previous REST request from polluting the GQL URL.
- **JSON connector `select = ""`** — an empty `select` field is treated as root selection (no path navigation), consistent with omitting the field.

---

## [0.6.0] — 2026-06-23 — Campaigns TUI & Assertions

### Added
- **Campaigns TUI tab** — 5th tab (after History) listing all `.toml` campaign files found in `<terapi_dir>/campaigns/`. Left panel shows the campaign list with step counts; right panel shows campaign metadata at idle, live step-by-step progress while running, and a full colour-coded report when done. `r` runs the selected campaign, `Esc` clears the result. Streaming architecture: `run_streaming()` sends `CampaignEvent`s over an async channel; `tick()` polls and updates the UI. The CLI `run` command now reuses the same streaming engine.
- **Campaign `continue_on_error`** — `continue_on_error = true` at campaign level (default for all steps) or step level (overrides campaign). A non-blocking step that fails is marked `✗ [continu]` in the CLI output and `✗ [↷]` in the TUI; the pipeline continues but extracted variables are not propagated. Exit code remains `1` if any step fails.
- **Campaign assertions** — `assert = [...]` field on campaign steps: validate status code, response body fields, headers, and elapsed time. Operators: `eq`, `ne`, `lt`, `lte`, `gt`, `gte`, `in`, `exists`, `contains`, `matches` (regex). `{{VAR}}` placeholders resolved in assertion values. Adds `regex` crate dependency.
- **Assertion visualization in TUI** — Idle panel shows each step's assertions as `?` hints. Running and Done panels show all assertions with `✓` (green) / `✗` (red) in real time after each step completes.
- **Campaign transform steps** — `kind = "transform"` step type runs data transformations without HTTP. Types: `template`, `regex`, `replace`, `split`, `trim`, `upper`, `lower`. Appear as `TRSF` in the output.
- **Universal `terapi import`** — auto-detects whether the file is a collection or a campaign TOML and places it in the right directory (`collections/` or `campaigns/`).

### Fixed
- **`continue_on_error` TOML placement** — the field belongs at root level (before `[campaign]`), not inside the `[campaign]` table. Documentation corrected.
- **Assertion result storage** — `StepResult` now stores all assertions as `Vec<(description, passed)>` instead of failures only; CLI report still shows failures only.

---

## [0.5.0] — 2026-06-23 — GraphQL native

### Fixed
- **Raw response view — word wrap** — long lines now wrap to the panel width instead of being clipped horizontally. `↑`/`↓` still scroll one visual (wrapped) line at a time.
- **Low-contrast gray** — `Color::DarkGray` (ANSI 8, near-invisible on dark terminals) replaced by `Color::Indexed(242)` throughout the UI: separators (`·`, `=`, `:`), unselected cursor markers, JSON `null` values, and unselected Options rows.

### Changed
- **Quit behaviour** — `q` now requires a second press to exit: the first press shows `Press q again to quit` in yellow in the status bar; any other key cancels. `Esc` is no longer a quit shortcut — it only closes modals or exits edit modes; at the top level it does nothing.

### Added
- **GraphQL mode** — `g` on the Request tab toggles between REST and GraphQL mode; the URL bar shows a magenta `GQL` badge and the method selector is hidden
- **GraphQL sub-tabs** — Query | Variables | Headers | Schema | Options replace the REST sub-tabs when GraphQL mode is active
- **Query editor** — tui-textarea with magenta border; `i` to enter, `Esc` to exit; `{{VAR}}` auto-completion via var picker works in the query textarea
- **Variables tab** — key/value list (`a` add, `d` delete, `Enter` edit); serialised as a flat JSON object and merged into the request body at send time
- **Auto-inject Content-Type** — `Content-Type: application/json` added automatically if absent when sending a GraphQL request
- **GraphQL TOML fields** — `graphql = true`, `graphql_query`, `graphql_variables` in the collection TOML format (`#[serde(default)]` keeps existing collections backward-compatible)
- **Collections tree** — GraphQL requests display a magenta `GQL` badge instead of the HTTP method
- **Breadcrumb** — `GraphQL › Query` (etc.) shown in the context bar when GraphQL mode is active
- **`g` to return to REST** — pressing `g` in GraphQL mode switches back to REST without clearing the URL or headers
- **Schema introspection** — Schema sub-tab now live: `f` sends a shallow `{ __schema { types { name kind } } }` query and displays all user-defined types in a scrollable list (left panel); `Enter` on a selected type fires a `__type` detail query and shows its fields, arg types, and enum values in the right panel; two-phase design keeps each query at depth ≤ 3 to pass CDN depth limits
- **New example collections** — `examples/collections/rick-morty-graphql.toml` (6 folders, 17 requests; Rick & Morty API — variables, pagination, multi-ID, aliases, filters, introspection) and `examples/collections/countries-graphql.toml` (5 folders, 19 requests; Countries API — filters, glob, inline fragments, introspection)

---

## [0.3.0] — 2026-06-22 — Collections, Environments & Polish

### Changed
- **Tab order** — Collections is now the first tab (Collections → Request → Env → History) and the default landing tab on startup. The most common workflow is to browse collections and load a request, which auto-switches to Request; starting on Collections saves one `Tab` press on every launch.
- **Unresolved `{{VAR}}` warning** — when the current request contains `{{VAR}}` placeholders but no environment is active, the top-right indicator switches from `○ no active env` to `⚠ {{VAR}} not resolved` (yellow). At send time the status bar also prefixes `⚠ unresolved {{VAR}} —` to the sending message. Scans URL, headers, URL params, body (text and JSON), and all auth fields.
- **Edit request from Collections** — pressing `e` on a request node now loads the request fully into the Request tab (instead of opening a limited modal). All fields are editable: URL, method, headers, URL params, body, auth, and **description**. Press `S` to open the **Update Request** modal pre-filled with the original name and location:
  - Keep location → saves in place (rename supported: just edit the Name field)
  - Change collection or folder → saves as a new entry at the new location (original preserved)
  - Press `n` to discard and start a new blank request instead
- **Description sub-tab** — now a real editable textarea (replaces the static placeholder). Press `i` to enter edit mode (border turns green), `Esc` to exit. Description is persisted in the collection TOML and restored when loading a request from Collections.
- **Response panel** — takes 2/3 of the available height (up from 1/2), giving more room to inspect responses.
- **`S: save` hint** — shown in the status bar on every Request sub-tab (was previously missing from Headers, URL Params, Body, Auth, and Options).
- **Options sub-tab** — now has four configurable options navigable with `↑`/`↓`; `Space`/`Enter` toggles or cycles the selected option:
  - **Skip TLS verification** — accept self-signed / hostname-mismatched certificates (existing)
  - **Follow redirects** — automatically follow 3xx responses (up to 10 hops); default on
  - **Timeout** — cycles through presets: 5 / 10 / 15 / 20 / **30** / 45 / 60 / 90 / 120 / 300 s; default 30 s
  - **Cookie jar** — when enabled, stores received `Set-Cookie` headers and re-sends cookies on subsequent requests (session mode); jar is cleared when disabled or when starting a new request (`n`)
  - All four options are persisted in the collection TOML and restored when loading a request
- **Persistent HTTP client** — `reqwest::Client` is now kept alive in `App` and reused across requests (previously rebuilt on every send). The shared connection pool improves performance on repeated requests to the same host, and the cookie jar survives between sends when enabled.
- **User-Agent header** — all outgoing requests automatically include `User-Agent: terapi/<version>` (e.g. `terapi/0.3.0`). The value can be overridden per-request by adding a custom `User-Agent` header in the Headers sub-tab.

### Added
- **`terapi import <file.toml>`** — CLI subcommand to import a collection TOML file directly into the terapi collections directory:
  - Validates that the file is readable and is a valid collection TOML (same format as `examples/collections/`)
  - Derives the destination filename from the collection name (`sanitize_filename`)
  - Prints `Imported "<name>" → <path>` on first import or `Updated "<name>" → <path>` if a file with the same name already existed

- **Example collections — open data français** (`examples/collections/`) :
  - `sncf.toml` — API SNCF/Navitia : 6 dossiers, 20 requêtes (couverture, gares, horaires, itinéraires, réseau, temps réel) — auth Basic `{{SNCF_TOKEN}}`
  - `france-geo.toml` — API Géo + API Adresse IGN : 4 dossiers, 19 requêtes (communes, départements, régions, géocodage, géocodage inverse) — sans authentification
  - `france-eau.toml` — Hub'Eau (eaufrance.fr) : 3 dossiers, 19 requêtes (hydrométrie temps réel, qualité rivières, qualité nappes souterraines) — sans authentification
  - `france-meteo.toml` — API Météo-France : 4 dossiers, 17 requêtes (prévisions horaires, observations, pluie radar, vigilance J/J+1) — Bearer `{{METEO_TOKEN}}`

- **Request authentication** — Auth sub-tab is now fully interactive with four modes:
  - **No Auth** (default) — no authentication header added
  - **Bearer** — injects `Authorization: Bearer <token>`; token field editable via modal
  - **Basic** — prompts for username and password, encodes as Base64 and injects `Authorization: Basic …`; password masked with bullets in the UI
  - **API Key** — key name and value configurable; location togglable between **Header** and **Query Param** (appended to URL at send time)
  - `↑` / `↓` to navigate between fields, `Space` / `Enter` on the Type row to cycle auth type, `Enter` on a field to open an edit modal (cyan border)
  - `{{VAR}}` substitution applied to all auth field values at send time
  - Auth config persisted in `StoredRequest.auth` in the collection TOML (backward-compatible — existing files load as No Auth)

- **Persistent request history** — every sent request is recorded in `<terapi_dir>/history.toml` (max 100 entries, newest first):
  - History tab replaces the placeholder with a live list showing: timestamp (UTC) / method / status / elapsed / URL
  - Status codes are colour-coded: green 2xx, yellow 3xx/4xx, red 5xx, gray for transport errors
  - Both successful and failed requests are recorded
  - `↑` / `↓` to navigate, `Enter` to load the request back into the Request tab (method, URL, headers, body restored), `d` to delete an entry
  - Storage: `HistoryEntry` struct with timestamp, method, url, headers, body, status, elapsed_ms, response_body

- **Edit existing request** — press `e` on a request node in the Collections panel to open an **Edit Request** modal (cyan border) pre-filled with the current name, method and URL:
  - `Tab` to cycle between Name and URL fields
  - `←` / `→` to change the HTTP method
  - `Enter` to save — updates the request in place in the collection file
  - `Esc` to cancel without changes
  - Headers and body are preserved unchanged

### Fixed
- **`{{VAR}}` substitution in body at send time** — variables were already resolved in URL and headers but the body was sent verbatim. `resolve_vars()` is now applied to the body string before the request is dispatched.

### Added
- **`{{` variable auto-completion** — typing `{{` in any editable field opens a picker overlay showing variables from the active environment:
  - Available in: URL bar, header values, URL param values, body JSON field values, body text (textarea)
  - `↑` / `↓` to navigate, `Enter` to insert `{{VAR_NAME}}`, `Esc` to close without inserting
  - Typing characters after `{{` filters the list in real time
  - `Backspace` with no filter prefix removes one `{` and closes the picker
  - If no environment is active: a status bar message prompts to activate one in the Env tab
  - If the active environment has no variables: a status bar message says so

### Added
- **HTTP Exchange view** — press `r` to cycle through three response views:
  - **JSON** — parsed JSON tree (existing)
  - **Raw** — raw response body (existing)
  - **HTTP** — full HTTP exchange showing request and response in wire format:
    ```
    ── Request ──────────────────────────────────────────
    POST /login HTTP/1.1
    Host: api.tsodev.fr
    Content-Type: application/json
    Content-Length: 45

    {"username":"thierry","password":"Pr0bleme#"}

    ── Response ─────────────────────────────────────────
    HTTP/1.1 200 OK
    Content-Type: application/json
    Date: Tue, 02 Jun 2026 08:34:44 GMT

    {"token":"eyJ0eXAiOiJKV1Qi…"}
    ```
  - Request snapshot captures the fully resolved URL, headers (with `{{VAR}}` substituted), and body at send time
  - Response section shows all response headers + body
  - Useful for debugging: see exactly what was sent and what came back

### Added
- **Content-Type value picker** — selecting `Content-Type` in the header picker opens a second picker with 9 common values:
  - `application/json`, `application/x-www-form-urlencoded`, `multipart/form-data`
  - `text/plain`, `text/html`, `text/xml`, `application/xml`
  - `application/octet-stream`, `application/graphql`
  - **Custom…** — opens the modal with an empty value field
  - `Esc` goes back to the header picker
- **Header picker** — pressing `a` in the Headers sub-tab now opens a picker of common HTTP headers before the edit modal:
  - `Authorization` (pre-filled: `Bearer `)
  - `Content-Type` (pre-filled: `application/json`)
  - `Accept`, `Accept-Language`, `Accept-Encoding`, `Cache-Control`
  - `X-API-Key`, `X-Request-ID`, `User-Agent`, `Origin`, `Referer`
  - **Custom…** — opens the blank modal to type any header name
  - Selecting a common header opens the edit modal with the key pre-filled and the cursor on the value field (default value pre-filled where applicable)
- **Options sub-tab — Skip TLS verification** — navigate to the Options sub-tab (`←/→`) and press `Space` or `Enter` to toggle TLS certificate verification on/off:
  - `[ ] Skip TLS verification` (default) — strict cert validation
  - `[x] Skip TLS verification` (yellow) — accepts self-signed and hostname-mismatched certificates
  - Useful for local dev servers, VPN endpoints, or APIs with custom/internal certificates

### Fixed
- **Full error chain on connection failure** — transport errors (TLS, DNS, connection refused) now display the complete `caused by:` chain in Raw view instead of just the top-level message, making it possible to diagnose the actual cause (e.g. `caused by: A host name mismatch has occurred`)
- **Persistent context bar** — a permanent second status line now appears at the bottom of every screen:
  - Left: breadcrumb of the current context (`Request › Body › JSON › editing`, `Env › Variables`, …)
  - Right: active environment indicator — `● env: Production` (green) when an env is active, `○ no active env` (dim) otherwise
  - The existing keybinding hints line is preserved below it
- **Connection errors auto-switch to Raw view** — when a request fails at the transport layer (TLS, DNS, connection refused), the response area now automatically switches to Raw view so the full error message is visible immediately, instead of going through the JSON parser which only showed a generic parse error
- **URL edit mode — method cycling moved to `↑/↓`** — `←` / `→` in URL edit mode now navigate sub-tabs (matching the behaviour outside URL mode), while `↑` / `↓` cycle the HTTP method; this removes the conflict where `←/→` blocked sub-tab navigation while in the URL bar

### Added (continued)
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
- **New request (`n`)** — resets all fields in the Request tab (URL, method, headers, URL params, body, response) ready for a blank request
- **Save to collection (`S`)** — saves the current request state to a collection from the Request tab:
  - Modal with three fields: Name (free text), Collection (↑/↓ to cycle, `n/total` indicator), Folder (↑/↓ to cycle including root)
  - `Tab` to move between fields, `Enter` to save, `Esc` to cancel
  - Saves method, URL + query params, headers, and body; writes to disk immediately
  - Status bar confirms the collection saved to
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
