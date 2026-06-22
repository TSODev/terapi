# terapi

**Terminal + API** — a keyboard-driven TUI for exploring, testing, and automating REST and GraphQL APIs, without leaving your terminal.

```
┌─────────────────────────── terapi ────────────────────────────┐
│  Request  |  Collections  |  Env  |  History                   │
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

- **GraphQL native** — schema introspection, query autocompletion, variable editing
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
| `m` | Cycle HTTP method |
| `s` | Send request |
| `i` | Edit body (Body sub-tab — enter editor) |
| `t` | Toggle body mode: Text ↔ JSON (Body sub-tab, outside editor) |
| `←` / `→` | Navigate sub-tabs (or cycle method in URL mode) |
| `↑` / `↓` | Move response cursor (JSON) / scroll (Raw) / navigate JSON body fields |
| `Enter` | Send request (URL mode) / fold-unfold JSON node / edit body field (JSON mode) |
| `Esc` | Exit URL mode / exit body editor |
| `r` | Toggle JSON ↔ Raw response view |
| `-` / `=` | Resize Key column |
| `q` / `Esc` | Quit |

**Collections panel**

| Key | Action |
|-----|--------|
| `Tab` | Switch panel |
| `↑` / `↓` | Move cursor |
| `Enter` | Expand / collapse folder |
| `n` | New collection |
| `f` | New folder in selected collection |
| `a` | Add request to selected collection / folder |
| `d` | Delete selected item |
| `q` / `Esc` | Quit |

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
| `q` / `Esc` | Quit |

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

---

## Campaign runner

Terapi includes a headless campaign runner for API automation.

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

## Roadmap

### v0.2 — REST basics *(in progress)*
- [x] Method selector — `m` to cycle, `←/→` in URL edit mode
- [x] URL input — `e` to edit, `Enter` to send, `Esc` to cancel
- [x] Send request — async via tokio, `{{VAR}}` resolved from active env
- [x] Response viewer: status code (color-coded) + elapsed time in title
- [x] Headers editor — `a`/`d`/`↑↓` in Headers sub-tab, `{{VAR}}` resolved before send
- [x] Body editor — dual mode: Text (`tui-textarea`) and JSON key/value table; `t` to toggle, `i` to edit

### v0.3 — Collections
- [x] TOML-based collection format (one file per collection)
- [x] Load collections from disk at startup
- [x] Create / delete collections from the TUI (`n`, `d`)
- [x] Create folders within a collection (`f`) — cursor follows automatically
- [x] Add / delete requests from the TUI (`a`, `d`)
- [x] Select a request from Collections to load into the Request tab (`Enter`)
- [ ] Edit an existing request (name, method, URL, headers, body)

### v0.4 — Environment & History
- [x] Environment tab — create / manage multiple environments (test, staging, prod…)
- [x] Add variables per environment (`KEY = value`), activate with `Enter`
- [x] Storage: one TOML file per environment in `<terapi_dir>/envs/`
- [x] Active env indicator in Request panel URL bar (`· env: <name>`)
- [x] Campaign: `env_file = "name"` loads a terapi env as base variable set
- [x] Campaign: per-step `env = "name"` to switch environment for a single step
- [ ] Inject active environment variables into requests (`{{VAR}}` substitution in TUI, requires editable URL/body)
- [ ] Request history (persistent, TOML)

### v0.5 — GraphQL
- [ ] GraphQL mode toggle
- [ ] Schema introspection via `__schema`
- [ ] Query editor with field autocompletion
- [ ] Variables panel (JSON)
- [ ] Mutations support

### v0.6 — Automation / Scripting
- [ ] Assertions (status code, body field values)
- [ ] Import from Postman collection (JSON v2.1)

### v1.0
- [ ] Auth: Bearer token, API Key, OAuth2 (basic) — via the Auth tab
- [ ] Syntax highlighting (syntect)

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
