# terapi

[![crates.io](https://img.shields.io/crates/v/terapi.svg)](https://crates.io/crates/terapi)
[![Downloads](https://img.shields.io/crates/d/terapi.svg)](https://crates.io/crates/terapi)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-2021-orange.svg)](https://www.rust-lang.org)

**Terminal + API** — a keyboard-driven TUI for exploring, testing, and automating REST and GraphQL APIs, without leaving your terminal.

![terapi — GraphQL mode: query editor + JSON response tree](https://raw.githubusercontent.com/TSODev/terapi/main/assets/screenshots/terapi-graphql.png)

## Why terapi?

| Tool | Problem |
|------|---------|
| Postman / Insomnia | Electron, cloud account required, heavy |
| ATAC | Great REST TUI, but no GraphQL, no scripting |
| hurl | Excellent for scripting, no interactive TUI |
| HTTPie | Terminal, but not TUI |

**terapi** aims to be all of the above in one tool:

> **Full documentation** → [USAGE.md](https://github.com/TSODev/terapi/blob/main/USAGE.md)

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
terapi                              # launch TUI
terapi --demo response.json         # launch TUI with a JSON file pre-loaded
terapi build                        # open campaign builder (blank)
terapi build my_campaign.toml       # open campaign builder (existing file)
terapi run campaign.toml                     # run a campaign headlessly
terapi run campaign.toml -p KEY=VAL          # override a [[params]] value
terapi run campaign.toml --silent            # exit 0/1 only (CI/cron)
terapi run campaign.toml --only "Login"      # run only the named step(s)
terapi run campaign.toml --format json       # machine-readable JSON output
terapi run campaign.toml --format csv        # CSV output (one row per step)
terapi run campaign.toml --retry 3           # retry failed steps (exp. backoff)
terapi import file.toml             # import a collection or campaign TOML
terapi --version
terapi --help
```

---

## Import — Postman & Insomnia

`terapi import <file.json>` auto-detects the format and copies the result to the right directory:

```bash
terapi import my_collection.json        # Postman v2.1 collection or environment
terapi import insomnia_export.json      # Insomnia v4 export
```

After import, a report is printed:

```
✓ Postman v2.1 — "My API"  →  ~/.config/terapi/collections/my-api.toml
  Requests  : 23
  Folders   : 5
  Env       : "My API vars"  →  ~/.config/terapi/envs/my-api-vars.toml  (8 vars)
  ! 2 formdata step(s) degraded to raw body
  ! 1 script(s) ignored (pre/post-request scripts not supported)
```

**Supported:**
- Postman v2.1 — collections (folders, requests, auth, headers, body, raw/GraphQL/formdata) + environment files; collection variables saved as a separate terapi env
- Insomnia v4 — collections (nested folders, GraphQL, auth) + base environments and sub-environments merged; gRPC/WebSocket entries counted but skipped

**Auth mapping:** Bearer → Bearer · Basic → Basic · API Key → API Key · OAuth2 → OAuth2 Client Credentials

---

## TUI keybindings

| Key | Action |
|-----|--------|
| `Tab` | Cycle panels forward |
| `Shift+Tab` | Cycle panels backward |

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
| `Space` / `Enter` | Auth sub-tab (Type row) — cycle auth type (No Auth → Bearer → Basic → API Key → OAuth2 CC → OAuth2 AC) |
| `Enter` | Auth sub-tab (field row) — open edit modal for token / username / password / key / OAuth2 fields |
| `f` | Auth sub-tab — fetch OAuth2 token manually (without sending the request) |
| `Esc` | Auth sub-tab — cancel OAuth2 browser wait or clear OAuth2 error |
| `↑` / `↓` | Options sub-tab — navigate between options |
| `Space` / `Enter` | Options sub-tab — toggle (Skip TLS / Follow redirects / Cookie jar) or cycle timeout |
| `r` | Cycle response view: JSON → Raw → HTTP (full diagnostics + redirect chain + cookies) |
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
| `e` | Edit selected request — loads into Request tab with all fields editable; `S` opens Update Request modal pre-filled with name/collection/folder |
| `D` | Duplicate selected request — loads all fields, opens Save modal with `"<name> copy"` in same collection/folder, saves as new entry |
| `E` | Open collection TOML in `$EDITOR` — TUI suspends, reloads on exit |
| `/` | Search / filter the collection tree — type to narrow the list, `↑`/`↓` navigate, `Enter` loads, `Esc` closes |
| `d` | Delete selected item |
| `q` `q` | Quit (press twice to confirm) |

**Env panel**

| Key | Action |
|-----|--------|
| `Tab` | Switch panel |
| `←` / `→` | Switch focus: Environments ↔ Variables |
| `↑` / `↓` | Navigate within focused panel |
| `Enter` | Activate selected environment (focus left) / Edit selected variable (focus right) |
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
| `↑` / `↓` | Navigate campaign list (List focus) — or move step cursor (Done panel, Result focus) |
| `r` | Run selected campaign — or open params modal if `[[params]]` defined |
| `L` | Load selected step into Request tab (Done panel, Result focus) |
| `E` | Open campaign TOML in `$EDITOR` — TUI suspends, reloads on exit |
| `Esc` | Clear run result |
| `q` `q` | Quit (press twice to confirm) |

---

## Campaigns panel

The Campaigns tab lists all `.toml` campaign files found in `<terapi_dir>/campaigns/` and lets you run them interactively without leaving the terminal.

![terapi — Campaigns tab: step preview + parameter popup before run](https://raw.githubusercontent.com/TSODev/terapi/main/assets/screenshots/terapi-campaigns.png)

The right panel has three states:
- **Idle** — campaign metadata (name, description, step list) and a `r` reminder
- **Running** — each completed step appears immediately; `⟳ current step…` shows what is in flight
- **Done** — colour-coded verdict (`✓ ALL PASSED` / `✗ SOME STEPS FAILED`), per-step results, extracted variables, assertion failures

Place campaign files in the campaigns directory:

```bash
# Global
cp examples/campaigns/crud_demo.toml ~/.config/terapi/campaigns/

# Per-project
mkdir -p .terapi/campaigns
cp examples/campaigns/transform_demo.toml .terapi/campaigns/

# Or use the import command (auto-detects collection vs campaign)
terapi import examples/campaigns/crud_demo.toml
```

---

## HTTP view — debugging

Press `r` twice from the response area to reach the **HTTP view**. It shows the complete exchange in wire format — useful when you need to see exactly what was sent and what came back:

```
── Request ──────────────────────────────────────────────
POST /login HTTP/1.1
Host: api.tsodev.fr
Content-Type: application/json
Cookie: session=abc123            ← jar cookies when cookie jar is enabled
Content-Length: 45

{"username":"thierry","password":"Pr0bleme#"}

── Response ─────────────────────────────────────────────
HTTP/1.1 200 OK
Content-Type: application/json

{"token":"eyJ0eXAiOiJKV1Qi…"}

── Redirects ────────────────────────────────────────────   ← 3xx hops (when follow redirects on)
  1  301 → https://www.example.com/login

── Cookies ──────────────────────────────────────────────   ← Set-Cookie details
  session=abc123  ; Path=/; HttpOnly

── Diagnostics ──────────────────────────────────────────
  Elapsed     84 ms                   ← green <300ms / yellow <1s / red ≥1s
  Size        1.2 KB
  Type        application/json; charset=utf-8
  Server      nginx/1.24.0
```

Transport errors (DNS failure, TLS error, timeout) are displayed inline with the full `caused by:` chain.

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

See `examples/collections/collection.toml` for a fully annotated template.

### Example collections

Ready-to-use collections in `examples/collections/` — copy them to your terapi directory to get started immediately:

| File | Contenu | Auth |
|------|---------|------|
| `public-rest.toml` | JSONPlaceholder, ReqRes, httpbin, PokeAPI, CoinGecko | Aucune |
| `graphql.toml` | Countries API, Rick & Morty API (POST GraphQL) | Aucune |
| `rick-morty-graphql.toml` | Rick & Morty API — personnages, épisodes, lieux, filtres, pagination, introspection | Aucune |
| `countries-graphql.toml` | Countries API — pays, continents, langues, filtres, introspection | Aucune |
| `spacex-graphql.toml` | SpaceX — company, rockets, dragons, ships, launches, roadster, cores, capsules, missions (~20 requêtes) | Aucune |
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

Variable priority (lowest → highest): `env_file` → `[env]` → `[[params]]` defaults → connector row → step `env` → extracted vars → runtime overrides.

### Campaign parameters

`[[params]]` declares user-facing inputs with a description and a default. Params can be overridden from the CLI (`-p KEY=VALUE`) or the TUI params modal — without touching the TOML:

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

```bash
terapi run itineraire_demo.toml -p DEPART=Bordeaux -p ARRIVEE=Nantes
```

In the TUI, pressing `r` on a campaign with `[[params]]` opens an interactive form to fill in values before running.

### Campaign pipeline

Data flows through five stages — each one is optional:

```
[[params]]  →  [env_file / env]  →  [[connectors]]  →  [[steps]]  →  [[outputs]]
  user           base vars            rows (CSV /        HTTP /          write JSON
  inputs                              JSON file /        transform /     to disk
                                      seed step)         assertions
```

**Input connectors** — run the campaign once per row:

```toml
# CSV file — column names become {{variables}}
[[connectors]]
type = "csv"
path = "contacts.csv"

# JSON file — iterate over an array
[[connectors]]
type   = "json"
path   = "users.json"
select = "users"        # dot-path to the array (omit for root)

# Seed step — use an HTTP response as the data source (no file needed)
[[connectors]]
type      = "json"
from_step = "Fetch items"   # name of the seed step below
select    = ""              # empty = root of the response

[[steps]]
name   = "Fetch items"
kind   = "seed"             # runs once before the loop
method = "GET"
url    = "https://api.example.com/items"
```

**Transform steps** — reshape variables between HTTP steps:

```toml
[[steps]]
name = "Extract ID"
kind = "transform"
transforms = [
  { type = "regex",    input = "{{LOCATION}}", pattern = "/items/(\\d+)", group = 1, output = "ITEM_ID" },
  { type = "template", input = "{{FIRST}} {{LAST}}",                                 output = "FULL_NAME" },
]
```

**Output connectors** — write step results to disk after all iterations:

```toml
[[outputs]]
from_step = "Fetch items"           # step whose response body to collect
path      = "/tmp/items.json"       # written as a JSON array, one element per iteration
select    = "data"                  # optional: extract a sub-field before writing
```

Outputs can be chained — the file written by campaign A becomes the `path` of campaign B's JSON connector.

### Variable extraction

Extracted values use dot-path notation over the JSON response:

| Path | Extracts |
|------|----------|
| `token` | `response["token"]` |
| `user.id` | `response["user"]["id"]` |
| `data.items.0.name` | `response["data"]["items"][0]["name"]` |
| `data.*.id` | all `id` fields from the `data` array → stored as a JSON array |

### foreach — iterate over an extracted array

`foreach` runs a step once per element of an extracted JSON array. Use `{{item}}` for the current element and `{{item_index}}` for its 0-based position:

```toml
[[steps]]
name    = "List users"
url     = "https://api.example.com/users"
[steps.extract]
user_ids = "*.id"          # wildcard: collects all id fields → [1,2,…,10]

[[steps]]
name    = "Get profile"
foreach = "{{user_ids}}"   # iterates over each element
url     = "https://api.example.com/users/{{item}}/profile"
```

When each element is itself an array (e.g. `[lon, lat]`), terapi automatically injects `{{item_0}}`, `{{item_1}}`, … into the iteration env. When it is an object, fields are accessible as `{{item_fieldname}}`:

```toml
[steps.extract]
coords = "features.*.geometry.coordinates"   # → [[lon0,lat0],[lon1,lat1],…]

[[steps]]
foreach = "{{coords}}"
url     = "https://api.example.com/reverse?lon={{item_0}}&lat={{item_1}}"
```

- Each iteration streams live: `✓ Get profile [3/10]`
- The step shows a `↻` badge in the Campaign panel idle view
- `continue_on_error` and `assert` apply per iteration
- Output connector collects all N bodies into the JSON array

### Conditional steps (`when`)

Add `when` to any step to make its execution depend on a campaign variable. If the condition is false, the step is skipped (`⊘ skipped`) without failing the pipeline:

```toml
[[steps]]
name    = "Get user"
url     = "https://api.example.com/users/{{ID}}"
[steps.extract]
USER_TYPE = "type"   # "premium" or "free"

[[steps]]
name   = "Premium flow"
when   = { var = "USER_TYPE", eq = "premium" }
method = "POST"
url    = "https://api.example.com/premium/activate"

[[steps]]
name   = "Retry if no token"
when   = { var = "TOKEN", exists = false }
method = "POST"
url    = "https://api.example.com/auth/refresh"
```

**Operators:** `eq` / `ne` / `exists = true|false` / *(no operator: var non-empty)*. Comparison values support `{{VAR}}`.

In the TUI idle view, steps with `when` show `⊘ if VAR == "value"` in grey below the step name.

### Search / Filter steps (`kind = "search"`)

Filter a JSON array stored in a variable by a regex applied to a field, and store the matches in a new variable:

```toml
[[steps]]
name   = "Filter active users"
kind   = "search"
search = {input = "{{USERS}}", path = "status", match = "^active$", output = "ACTIVE_USERS"}

[[steps]]
name   = "Find first premium email"
kind   = "search"
search = {input = "{{USERS}}", path = "email", match = "@premium\\.com$", output = "FIRST_PREMIUM", first_only = true}
```

| Field | Description |
|-------|-------------|
| `input` | Variable holding the JSON array (e.g. `{{USERS}}`) |
| `path` | Dot-path into each element to match against — empty to match on the element directly |
| `match` | Regex pattern |
| `output` | Variable name to store results (default `RESULTS`) |
| `first_only` | `true` → store only the first match (or `"null"` if none); `false` (default) → store JSON array of all matches |

The `SRCH` badge (cyan) appears in the Campaigns panel idle view and in the Campaign Builder pipeline.

---

### Campaign examples

Ready-to-run examples in `examples/campaigns/` — no API key required:

| File | What it demonstrates |
|------|----------------------|
| `crud_demo.toml` | All HTTP methods with assertions |
| `transform_demo.toml` | Transform steps: regex, template, upper, split |
| `seed_step_demo.toml` | Seed step + JSON connector + output connector |
| `itineraire_demo.toml` | **`[[params]]` + full pipeline**: geocode two cities, route via IGN, reverse-geocode each waypoint with `{{item_0}}/{{item_1}}`, output labelled step list |
| `eu_capitals.toml` | **4-step pipeline**: GraphQL seed (53 EU countries) → language transform → geocode capital → live weather (Open-Meteo); paired with `eu_capitals_map.html` |
| `foreach_demo.toml` | **`foreach`**: fetch user list, extract IDs with `*.id` wildcard, iterate over each user to fetch their todos |
| `when_demo.toml` | **`when`**: `eq` / `ne` / `exists` operators — admin vs standard user branches with automatic cascade |
| `loop_pagination_demo.toml` | **`kind = "loop"`**: two patterns — next-URL cursor (Rick & Morty) and last-ID-as-offset (JSONPlaceholder); collects all 100 posts in 4 pages |
| `spacex_exploration.toml` | **GraphQL pipeline**: company → fleet → latest launch → all 109 past launches (wildcard `*.id`) → roadster position → booster stats → summary transform |

```bash
terapi run examples/campaigns/crud_demo.toml
terapi run examples/campaigns/seed_step_demo.toml
terapi run examples/campaigns/eu_capitals.toml
terapi run examples/campaigns/itineraire_demo.toml -p DEPART=Bordeaux -p ARRIVEE=Nantes
terapi run examples/campaigns/loop_pagination_demo.toml
terapi run examples/campaigns/spacex_exploration.toml
```

#### Interactive weather map

`eu_capitals.toml` generates `examples/campaigns/eu_capitals_weather.json`. Open `examples/campaigns/eu_capitals_map.html` with a local server to visualize all EU capitals on a dark map — coloured bubble per capital (flag + temperature + weather icon), full detail popup on click:

```bash
terapi run examples/campaigns/eu_capitals.toml
python3 -m http.server 8080 --directory examples
open http://localhost:8080/eu_capitals_map.html
```

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
- `spacex-graphql.toml` — SpaceX community API — 8 folders, ~20 requests: company, rockets, dragons, ships, launches (latest/past/next/paginated/by-rocket), capsules, cores, missions, roadster, history, introspection

---

## OAuth2

Terapi supports two OAuth2 flows in the **Auth** sub-tab. Press `Space`/`Enter` on the Type row to cycle to **OAuth2 CC** (Client Credentials) or **OAuth2 AC** (Authorization Code).

**OAuth2 Client Credentials** — machine-to-machine, no browser:

| Field | Example |
|-------|---------|
| Token URL | `https://auth.example.com/oauth/token` |
| Client ID | `my-client` |
| Client Secret | `secret` |
| Scope | `api:read` (optional) |

Press `s` to send — terapi fetches the token automatically first, then fires the request. The token is cached for its `expires_in` lifetime. Press `f` to refresh the cache without sending.

**OAuth2 Authorization Code** — browser-based login:

| Field | Example |
|-------|---------|
| Token URL | `https://auth.example.com/oauth/token` |
| Client ID | `my-client` |
| Client Secret | `secret` |
| Scope | `openid profile` |
| Auth URL | `https://auth.example.com/oauth/authorize` |
| Redirect Port | `9876` (local TCP port for the callback) |

Press `f` — terapi opens your browser on the authorization URL, starts a local TCP listener on the redirect port, captures the authorization code when the browser redirects back, exchanges it for a token, and caches it. Then press `s` to send.

Auth config (all fields except the token itself) is saved in the collection TOML under `[auth]` with backward-compatible `#[serde(default)]`. The token is never written to disk — session only.

---

## Campaign Builder

`terapi build` — an interactive TUI campaign editor, built into the same binary. No extra install. Creating a campaign TOML by hand is powerful but tedious — the Builder turns it into a keyboard-driven experience:

```
┌─ Builder: my_campaign.toml * ────────────────────────────────────────────────┐
│  ┌─ Pipeline · users [production] ─┐  ┌─ ✓ Run result ─────────────────────┐ │
│  │  [CSV] contacts.csv             │  │  200  142 ms  /users                │ │
│  │  [1] HTTP  GET   /health        │  │  ✓ status eq 200                    │ │
│  │▶ [2] HTTP  POST  /users         │  │  ↳ USER_ID = 42                     │ │
│  │       ? status eq 201           │  │                                     │ │
│  │  [3] TRSF  upper → USERNAME     │  │  {                                  │ │
│  │  [4] FILE  avatar.png → B64     │  │    "id": 42,                        │ │
│  │       ↻ foreach: {{user_ids}}   │  │    "name": "Alice",                 │ │
│  │       ⊘ if ROLE == "admin"      │  │    …                                │ │
│  │  [5] WAIT  500ms                │  │  }                                  │ │
│  │  [OUT] results.json             │  │                                     │ │
│  └─────────────────────────────────┘  └─────────────────────────────────────┘ │
│  Builder › Step editor  —  ↑↓: field  Enter: edit  r: run step  Esc: back    │
└───────────────────────────────────────────────────────────────────────────────┘
```

```bash
terapi build                        # blank campaign
terapi build my_campaign.toml       # edit an existing file
```

**What's in the builder:**

- **Numbered pipeline** — steps with badges (`HTTP` `TRSF` `WAIT` `SEED` `FILE` `SRCH` `LOOP` `#`) and inline hints (`↻` foreach, `⊘` when, `?` assertions)
- **[IN] / [OUT] sections** — navigable connectors above steps and output blocks below
- **Brick catalog** — HTTP, Transform, Pause, Seed, File Loader, Search / Filter, Loop, Comment, Connector [IN], Output [OUT]
- **Step editor** — all fields for every step type; multi-line body textarea; assertions, when, foreach guided entry
- **Run step** (`r`) — execute the current step immediately; response shown in the right panel below the editor; status, assertions, extracted vars, body preview
- **JSON path autocomplete** (`Tab` on Extract value) — after running a step, picks dot-paths from the response JSON
- **Load from collection** (`L`) — browse existing collections and fill method/URL/headers/body in one keystroke
- **Variables panel** (`v`) — full CRUD on the `[env]` block
- **Checker** (`c`) — static validation: undefined `{{VAR}}`, empty URLs / file paths, invalid `from_step` references, duplicate step names
- **TOML preview** (`p`) — syntax-highlighted live preview (`[section]` cyan, `[[array]]` magenta, strings green)
- **Save** (`w`) — writes to the target file or `<terapi_dir>/campaigns/`
- **Quit confirmation** — `y / n / Esc` prompt when there are unsaved changes

> Full reference → [USAGE.md — Campaign builder](https://github.com/TSODev/terapi/blob/main/USAGE.md#campaign-builder)

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

## Feedback & contributions

Bug reports, feature requests, and questions are welcome:

- **GitHub issues** — [github.com/TSODev/terapi/issues](https://github.com/TSODev/terapi/issues)
- **Email** — [thierry.soulie@tsodev.fr](mailto:thierry.soulie@tsodev.fr)

---

## License

MIT — © [TSODev](https://github.com/TSODev)
