# terapi

[![crates.io](https://img.shields.io/crates/v/terapi.svg)](https://crates.io/crates/terapi)
[![Downloads](https://img.shields.io/crates/d/terapi.svg)](https://crates.io/crates/terapi)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-2021-orange.svg)](https://www.rust-lang.org)

**Terminal + API** — a keyboard-driven TUI for exploring, testing, and automating REST and GraphQL APIs, without leaving your terminal.

```
┌─────────────────────────── terapi ────────────────────────────┐
│  Collections  |  Request  |  Env  |  History  |  Campaigns     │
├────────────────────────────────────────────────────────────────┤
│ ┌─ URL ──────────────────────────────────────────────────────┐ │
│ │ GET  https://api.example.com/users                         │ │
│ └────────────────────────────────────────────────────────────┘ │
│  Description | Headers | URL Params | Body | Auth | Options    │
│ ┌─ JSON · Raw  r: toggle  -/=: resize ──────────────────────┐ │
│ │ ▼ (root)          Object                                   │ │
│ │   status          String   "success"                       │ │
│ │   code            Number   200                             │ │
│ │ ▼ data            Object                                   │ │
│ │   ▶ user          Object   { id: 42, username: "tsodev" …} │ │
│ └────────────────────────────────────────────────────────────┘ │
├────────────────────────────────────────────────────────────────┤
│ Request  ›  Body  ›  JSON             ● env: Production        │
│ Tab: panels  ←/→: section  ↑/↓: cursor  Enter: fold  q: quit  │
└────────────────────────────────────────────────────────────────┘
```

## Why terapi?

| Tool | Problem |
|------|---------|
| Postman / Insomnia | Electron, cloud account required, heavy |
| ATAC | Great REST TUI, but no GraphQL, no scripting |
| hurl | Excellent for scripting, no interactive TUI |
| HTTPie | Terminal, but not TUI |

**terapi** aims to be all of the above in one tool:

- **GraphQL native** — schema introspection, variable editing, collections save/load
- **Pipeline automation** — chain requests, extract variables, run campaigns headlessly
- **Local-first** — collections stored as TOML, git-friendly, no account, no cloud
- **Single binary** — `cargo install terapi`, instant startup, zero Electron

---

## Installation

```bash
cargo install terapi
```

Or build from source:

```bash
git clone https://github.com/tsodev/terapi
cd terapi
cargo build --release
./target/release/terapi
```

**Requirements:** Rust 1.75+ (edition 2021), any modern terminal with 256-color support.

---

## Usage

```bash
terapi                        # launch TUI (empty)
terapi --demo response.json   # launch TUI with a JSON file pre-loaded
terapi run campaign.toml            # run a campaign headlessly
terapi run campaign.toml --silent   # run silently — exit 0/1 only (CI/cron)
terapi import file.toml             # import a collection or campaign TOML
terapi --version
terapi --help
```

---

## TUI keybindings

**Request panel**

| Key | Action |
|-----|--------|
| `Tab` | Switch panel |
| `e` | Edit URL (enter URL mode) |
| `m` | Cycle HTTP method (outside URL mode) |
| `↑` / `↓` | Cycle HTTP method (in URL mode) / move response cursor / scroll |
| `n` | New request — clear all fields |
| `s` | Send request |
| `S` | Save current request to a collection |
| `i` | Edit description (Description sub-tab — enter editor) / Edit body (Body sub-tab — enter editor) |
| `a` / `d` | URL Params sub-tab — add / delete param |
| `t` | Toggle body mode: Text ↔ JSON (Body sub-tab, outside editor) |
| `←` / `→` | Navigate sub-tabs (also exits URL mode) |
| `Enter` | Send request (URL mode) / fold-unfold JSON node / edit body field (JSON mode) |
| `Esc` | Finish URL edit / exit body editor |
| `{{` | Open variable picker (any editable field) — insert `{{VAR}}` from active env |
| `↑` / `↓` | Auth sub-tab — navigate fields |
| `Space` / `Enter` | Auth sub-tab (Type row) — cycle auth type (No Auth → Bearer → Basic → API Key) |
| `Enter` | Auth sub-tab (field row) — open edit modal for token / username / password / key |
| `↑` / `↓` | Options sub-tab — navigate between options |
| `Space` / `Enter` | Options sub-tab — toggle (Skip TLS / Follow redirects / Cookie jar) or cycle timeout |
| `r` | Cycle response view: JSON → Raw → HTTP exchange |
| `-` / `=` | Resize Key column |
| `q` `q` | Quit (press twice to confirm) |

**GraphQL mode** (activate with `g`)

| Key | Action |
|-----|--------|
| `g` | Toggle GraphQL mode (REST ↔ GraphQL) |
| `←` / `→` | Navigate GraphQL sub-tabs (Query / Variables / Headers / Schema / Options) |
| `i` | Query tab — enter query editor |
| `Ctrl+Space` | Query tab — open autocompletion popup (fields / type names) |
| `Esc` | Query tab — exit query editor |
| `a` / `d` | Variables tab — add / delete variable |
| `Enter` | Variables tab — edit selected variable |
| `↑` / `↓` | Variables tab — navigate variables |
| `f` | Schema tab — fetch type list via introspection |
| `↑` / `↓` | Schema tab — navigate type list |
| `Enter` | Schema tab — load fields for selected type |
| `s` | Send GraphQL request |
| `S` | Save request to collection (query + variables preserved) |

**Collections panel**

| Key | Action |
|-----|--------|
| `Tab` | Switch panel |
| `↑` / `↓` | Move cursor |
| `Enter` | Expand / collapse folder — or load request into Request tab |
| `n` | New collection |
| `f` | New folder in selected collection |
| `a` | Add request to selected collection / folder |
| `e` | Edit selected request — loads into Request tab with all fields editable (URL, headers, body, auth, description); `S` opens Update Request modal (pre-filled name/location; change location to save as new) |
| `d` | Delete selected item |
| `q` `q` | Quit (press twice to confirm) |

**Env panel**

| Key | Action |
|-----|--------|
| `Tab` | Switch panel |
| `←` / `→` | Switch focus: Environments ↔ Variables |
| `↑` / `↓` | Navigate within focused panel |
| `Enter` | Activate selected environment |
| `n` | New environment |
| `a` | Add variable to selected environment |
| `d` | Delete selected environment or variable |
| `q` `q` | Quit (press twice to confirm) |

**History panel**

| Key | Action |
|-----|--------|
| `Tab` | Switch panel |
| `↑` / `↓` | Navigate entries |
| `Enter` | Load entry into Request tab (GraphQL entries restore GQL mode + query) |
| `d` | Delete entry |
| `q` `q` | Quit (press twice to confirm) |

**Campaigns panel**

| Key | Action |
|-----|--------|
| `Tab` | Switch panel |
| `↑` / `↓` | Navigate campaigns |
| `r` | Run selected campaign (live progress in right panel) |
| `Esc` | Clear run result |
| `q` `q` | Quit (press twice to confirm) |

---

## Campaigns panel

The Campaigns tab lists all `.toml` campaign files found in `<terapi_dir>/campaigns/` and lets you run them interactively without leaving the terminal.

```
┌─ Campaigns (2) ────────────────┐ ┌─ crud_demo ─────────────────────────────────────┐
│▶ crud_demo         (6 steps)   │ │  ✓ Create post     POST   201    312ms           │
│  transform_demo    (4 steps)   │ │  ✓ Read post       GET    200     98ms           │
│                                │ │  ✓ Update post     PUT    200    105ms           │
│                                │ │  ✓ Patch post      PATCH  200     87ms           │
│                                │ │  ✓ Delete post     DELETE 200     91ms           │
│                                │ │  ✓ Assert deleted  GET    404     77ms           │
│                                │ │                                                  │
│                                │ │  ✓  ALL PASSED  Steps: 6 ok / 0 failed  770ms   │
│                                │ │  Esc to clear  r to re-run                       │
└────────────────────────────────┘ └──────────────────────────────────────────────────┘
```

The right panel has three states:
- **Idle** — campaign metadata (name, description, step list) and a `r` reminder
- **Running** — each completed step appears immediately; `⟳ current step…` shows what is in flight
- **Done** — colour-coded verdict (`✓ ALL PASSED` / `✗ SOME STEPS FAILED`), per-step results, extracted variables, assertion failures

Place campaign files in the campaigns directory:

```bash
# Global
cp examples/crud_demo.toml ~/.config/terapi/campaigns/

# Per-project
mkdir -p .terapi/campaigns
cp examples/transform_demo.toml .terapi/campaigns/

# Or use the import command (auto-detects collection vs campaign)
terapi import examples/crud_demo.toml
```

---

## Collections

Collections are stored as TOML files — one file per collection. Terapi resolves the storage directory in priority order:

| Priority | Location | Use case |
|----------|----------|----------|
| 1 | `$TERAPI_DIR` | CI, cron, custom path |
| 2 | `./.terapi/collections/` | Per-project, versionable in Git |
| 3 | `~/.config/terapi/collections/` | Global, cross-project (default) |

### Collection TOML format

```toml
[collection]
name = "My API"
description = "Optional description"

[[folders]]
name = "Auth"

[[folders.requests]]
name = "Login"
method = "POST"
url = "https://api.example.com/auth/login"
body = '{"email": "{{EMAIL}}", "password": "{{PASSWORD}}"}'

[folders.requests.headers]
Content-Type = "application/json"

[[requests]]
name = "List users"
method = "GET"
url = "https://api.example.com/users"

[requests.headers]
Authorization = "Bearer {{TOKEN}}"
```

See `examples/collection.toml` for a fully annotated template.

### Example collections

Ready-to-use collections in `examples/collections/` — copy them to your terapi directory to get started immediately:

| File | Contenu | Auth |
|------|---------|------|
| `public-rest.toml` | JSONPlaceholder, ReqRes, httpbin, PokeAPI, CoinGecko | Aucune |
| `graphql.toml` | Countries API, Rick & Morty API (POST GraphQL) | Aucune |
| `rick-morty-graphql.toml` | Rick & Morty API — personnages, épisodes, lieux, filtres, pagination, introspection | Aucune |
| `countries-graphql.toml` | Countries API — pays, continents, langues, filtres, introspection | Aucune |
| `sncf.toml` | API SNCF — gares, horaires, itinéraires, perturbations | Basic `{{SNCF_TOKEN}}` |
| `france-geo.toml` | API Géo + API Adresse IGN — communes, départements, régions, géocodage | Aucune |
| `france-eau.toml` | Hub'Eau — hydrométrie, qualité rivières et nappes | Aucune |
| `france-meteo.toml` | Météo-France — prévisions, observations, vigilance | Bearer `{{METEO_TOKEN}}` |

```bash
# Copier une collection dans le répertoire global
cp examples/collections/france-geo.toml ~/.config/terapi/collections/

# Ou dans un projet local
mkdir -p .terapi/collections
cp examples/collections/sncf.toml .terapi/collections/
```

---

## Campaign runner

Terapi includes a headless campaign runner for API automation — and the same campaigns can be run interactively from the **Campaigns** TUI tab (see above).

### Campaign TOML format

```toml
[campaign]
name        = "Users API — smoke tests"
description = "Login then run CRUD operations"

# Optional: load a named terapi environment as base vars
env_file = "production"   # <terapi_dir>/envs/production.toml

[env]
BASE_URL = "https://api.example.com"   # overrides env_file if same key

[[steps]]
name   = "Login"
method = "POST"
url    = "{{BASE_URL}}/auth/login"
body   = '{"email": "admin@example.com", "password": "secret"}'
[steps.headers]
Content-Type = "application/json"
[steps.extract]
JWT     = "token"    # extracted from response JSON
USER_ID = "user.id"

[[steps]]
name   = "Get profile"
method = "GET"
url    = "{{BASE_URL}}/users/{{USER_ID}}"
[steps.headers]
Authorization = "Bearer {{JWT}}"

[[steps]]
name   = "Health check (staging)"
env    = "staging"   # uses staging terapi env for this step only
method = "GET"
url    = "{{BASE_URL}}/health"
```

Variable priority (lowest → highest): `env_file` → `[env]` → connector row → step `env` → extracted vars.

### Data-driven campaigns (CSV connector)

```toml
[[connectors]]
type = "csv"
path = "contacts.csv"   # columns become {{variables}}

# Campaign runs once per CSV row
[[steps]]
name   = "Invite contact"
method = "POST"
url    = "{{BASE_URL}}/invitations"
body   = '{"email": "{{contact_email}}", "name": "{{contact_name}}"}'
```

### Variable extraction

Extracted values use dot-path notation over the JSON response:

| Path | Extracts |
|------|----------|
| `token` | `response["token"]` |
| `user.id` | `response["user"]["id"]` |
| `data.items.0.name` | `response["data"]["items"][0]["name"]` |

### Campaign report

```
Campaign : Users API — smoke tests

  ✓ Login                  POST    200    142 ms
      ↳ JWT = eyJhbGciOiJIUzI1NiIs…
      ↳ USER_ID = 42
  ✓ Get profile            GET     200     89 ms
  ✗ Delete user            DELETE  404     34 ms  HTTP 404

╔════════════════════════════════════════════════════════════════════╗
║  Campaign Report — Users API — smoke tests                         ║
╠════════════════════════════════════════════════════════════════════╣
║  Steps      : 2 ok  /  1 failed  (3 total)                        ║
║  Duration   : 265 ms                                               ║
╠════════════════════════════════════════════════════════════════════╣
║  ✗  SOME STEPS FAILED                                              ║
╚════════════════════════════════════════════════════════════════════╝
```

---

## GraphQL mode

Press `g` on the Request tab to activate GraphQL mode. The URL bar shows a magenta **GQL** badge and the sub-tabs switch to GraphQL-specific tabs.

```
┌─────────────────────────── terapi ────────────────────────────┐
│  Collections  |  Request  |  Env  |  History  |  Campaigns     │
├────────────────────────────────────────────────────────────────┤
│ ┌─ GQL  https://countries.trevorblades.com/graphql ──────────┐ │
│ └────────────────────────────────────────────────────────────┘ │
│  Query | Variables | Headers | Schema | Options                │
│ ┌─ Query — i: edit ─────────────────────────────────────────┐ │
│ │ query CountryDetail($code: ID!) {                          │ │
│ │   country(code: $code) {                                   │ │
│ │     name  capital  currency  emoji                         │ │
│ │     continent { name }                                     │ │
│ │   }                                                        │ │
│ │ }                                                          │ │
│ └────────────────────────────────────────────────────────────┘ │
│ ┌─ 200 OK · 84 ms ──────────────────────────────────────────┐ │
│ │ ▼ data              Object                                 │ │
│ │ ▼ country           Object                                 │ │
│ │     name            String   "France"                      │ │
│ │     capital         String   "Paris"                       │ │
│ │     currency        String   "EUR"                         │ │
│ └────────────────────────────────────────────────────────────┘ │
├────────────────────────────────────────────────────────────────┤
│ GraphQL  ›  Query                    ● env: Production         │
│ i: edit query  s: send  S: save  ←/→: section  g: REST mode   │
└────────────────────────────────────────────────────────────────┘
```

| Sub-tab | Purpose |
|---------|---------|
| Query | Multi-line editor — `i` to edit, `Esc` to exit; `{{VAR}}` picker; `Ctrl+Space` to autocomplete |
| Variables | Key/value pairs serialised as the `variables` JSON object |
| Headers | Same header picker as REST mode |
| Schema | Schema browser — `f` fetch types, `↑/↓` navigate, `Enter` load fields |
| Options | Same options as REST mode (TLS, redirects, timeout, cookies) |

**Sending a GraphQL request:**
1. Press `e` to edit the endpoint URL
2. Press `←`/`→` to reach the **Query** tab, then `i` to write the query
3. Press `Ctrl+Space` to open the autocompletion popup — fields from the loaded schema type, or type names if no detail is loaded
4. Optionally switch to **Variables** (`←`/`→`) and press `a` to add variables
5. Press `s` — terapi posts `{"query": "...", "variables": {...}}` with `Content-Type: application/json` injected automatically

**Browsing the schema** (Schema tab):
1. Press `f` — fetches `{ __schema { types { name kind } } }` and shows all user-defined types on the left (OBJ / ENM / INP / INT / UNI badges)
2. Navigate with `↑`/`↓`, press `Enter` to load fields, arg types and return types on the right
3. Once a type is loaded, switch to the **Query** tab and press `Ctrl+Space` to complete field names from that type
4. Uses two shallow queries (depth ≤ 3) — works even on APIs with CDN query depth limits

**Collections** — press `S` to save. The TOML stores `graphql = true`, `graphql_query`, and `graphql_variables`. Loading a GQL request from Collections (`Enter` on the node) restores everything and activates GraphQL mode automatically. The node shows a magenta `GQL` badge in the tree.

Press `g` again to return to REST mode (URL and headers are preserved).

**Example GraphQL collections** in `examples/collections/`:
- `rick-morty-graphql.toml` — Rick & Morty API — 6 folders, 17 requests: variables, pagination, multi-ID, aliases, filters, introspection
- `countries-graphql.toml` — Countries API — 5 folders, 19 requests: filters, glob, inline fragments, introspection

---

## Stack

| Role | Crate |
|------|-------|
| TUI rendering | `ratatui` + `crossterm` |
| HTTP client | `reqwest` (async) |
| Async runtime | `tokio` |
| Serialization | `serde` + `serde_json` |
| Config / campaigns | `toml` |
| CSV connector | `csv` |
| CLI | `clap` |
| Config dir resolution | `dirs` |
| Error handling | `anyhow` |
| Body editor | `tui-textarea` |

---

## License

MIT — © [TSODev](https://github.com/tsodev)
