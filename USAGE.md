# Terapi — Usage Guide

## Table of contents

- [Installation](#installation)
- [TUI mode](#tui-mode)
  - [Panels](#panels)
  - [Request panel](#request-panel)
  - [Collections panel](#collections-panel)
  - [History panel](#history-panel)
  - [Keybindings](#keybindings)
- [Collections](#collections)
  - [Directory resolution](#directory-resolution)
  - [Collection TOML format](#collection-toml-format)
- [Demo mode](#demo-mode)
- [Campaign runner](#campaign-runner)
  - [Campaign TOML format](#campaign-toml-format)
  - [Variable substitution](#variable-substitution)
  - [Variable extraction](#variable-extraction)
  - [Data-driven campaigns (CSV)](#data-driven-campaigns-csv)
  - [Silent mode (CI/cron)](#silent-mode-cicron)

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

**Requirements:** Rust 1.75+, any modern terminal with 256-color support.

---

## TUI mode

Launch the TUI with no arguments:

```bash
terapi
```

### Panels

The interface is divided into three top-level panels, navigated with `Tab`:

| Panel | Description |
|-------|-------------|
| **Request** | Build and send HTTP requests, view responses |
| **Collections** | Browse saved collections and requests |
| **Env** | Create and manage environment variables across multiple environments |
| **History** | Recent requests *(coming in v0.4)* |

### Request panel

The Request panel is split into four zones, from top to bottom:

```
┌─ URL ─────────────────────────────────────────────────────────────┐
│   GET  https://api.example.com/users                               │
└───────────────────────────────────────────────────────────────────┘
```

In URL edit mode (`e`), the bar highlights and shows a cursor:

```
┌─ URL ─────────────────────────────────────────────────────────────┐
│ ◀ GET ▶  https://api.example.com/users_                            │
└───────────────────────────────────────────────────────────────────┘
```

**Sending a request:**
1. Press `e` to enter URL edit mode
2. Type the URL (Backspace to delete)
3. Use `←` / `→` to change the HTTP method
4. Press `Enter` to send — or `Esc` to cancel
5. Alternatively, press `s` at any time to send the current URL without entering edit mode

`{{VAR}}` placeholders in the URL are automatically resolved from the active environment (set in the Env tab) before the request is sent.

The response block title shows the **status code** (color-coded green/yellow/red) and **elapsed time** while the request is in flight, a `⟳ sending…` indicator is shown.

```
┌─ URL ──────────────────────────────────────────────┐
│ GET  https://api.example.com/users                  │
└─────────────────────────────────────────────────────┘
┌─ Description | Headers | URL Params | Body | Auth | Options ─┐
└──────────────────────────────────────────────────────────────┘
┌─ (Request content for the selected sub-tab) ────────┐
└─────────────────────────────────────────────────────┘
┌─ JSON · Raw  r: toggle  -/=: resize ────────────────┐
│  Key              Type     Value                     │
│  ▼ (root)         Object                             │
│    status         String   "success"                 │
│    code           Number   200                       │
│  ▼ data           Object                             │
│    ▶ user         Object   { id: 42, name: "…" }    │
└─────────────────────────────────────────────────────┘
```

**Sub-tabs** (navigate with `←` / `→`):

| Sub-tab | Purpose |
|---------|---------|
| Description | Free-text note about the request |
| Headers | Request headers (key / value) |
| URL Params | Query string parameters |
| Body | Raw JSON body editor |
| Auth | Bearer token, API Key, OAuth2 |
| Options | Timeout, redirects, SSL |

#### Body editor

Switch to the Body sub-tab (`←`/`→`), then press `i` to enter edit mode. The border turns green and a cursor appears:

```
┌─ Body  [editing — Esc to exit] ─────────────────────────────────┐
│ {                                                                  │
│   "email": "admin@example.com",                                   │
│   "password": "{{PASSWORD}}"                                      │
│ }                                                                  │
└────────────────────────────────────────────────────────────────── ┘
```

- Full text editing: arrows, Home/End, Backspace/Delete, multi-line
- `Esc` — exit editor (border returns to yellow)
- Body is sent with the request as-is; `{{VAR}}` placeholders are **not** yet resolved in the TUI body (coming soon — use the campaign runner for variable substitution in bodies)
- When the editor has content the title shows the line count: ` Body  (4 lines)  [i: edit] `
- An empty body sends no request body

**Response viewer** (bottom half of the Request panel):

The JSON view displays a 3-column table: **Key / Type / Value**.

- Objects and arrays show a `▼` / `▶` fold icon — press `Enter` to fold or unfold.
- Folded nodes display an inline content preview: `{ id: 42, name: "tsodev" … }`.
- Press `r` to toggle between **JSON** (structured table) and **Raw** (plain text) views.
- Use `-` / `=` to shrink or grow the Key column width.
- Use `↑` / `↓` to move the cursor row by row (JSON view) or scroll the raw text.

**Value type colours:**

| Colour | Type |
|--------|------|
| Cyan | Object |
| Blue | Array |
| Green | String |
| Yellow | Number |
| Magenta | Boolean |
| Dark grey | Null |

### Collections panel

Displays the full collection tree loaded from disk. Collections can contain folders (one level deep) and root-level requests.

**Navigation:**

- `↑` / `↓` — move cursor
- `Enter` — expand or collapse the selected folder

**Editing:**

| Key | Action |
|-----|--------|
| `n` | Create a new collection |
| `f` | Create a new folder inside the selected collection |
| `a` | Add a request to the selected collection or folder |
| `d` | Delete the selected item (collection, folder, or request) |

**Creating a collection (`n`)** — a modal prompts for a name. Press `Enter` to save or `Esc` to cancel. The collection is immediately written to disk.

**Creating a folder (`f`)** — a modal prompts for a name. The folder is added to the collection that contains the currently selected item. After creation, the cursor moves automatically to the new folder, so you can press `a` right away to add a request into it.

Typical workflow:
```
n   → new collection       (cursor on the collection)
f   → new folder Auth      (cursor moves to Auth)
a   → add request Login    (added inside Auth)
f   → new folder Users     (cursor moves to Users)
a   → add request List     (added inside Users)
```

**Adding a request (`a`)** — a modal with three fields:
- **Name** — displayed in the tree
- **Method** — cycle with `←` / `→` (GET / POST / PUT / PATCH / DELETE)
- **URL** — full URL, supports `{{VAR}}` placeholders

Use `Tab` to switch between Name and URL fields. Press `Enter` to save (both fields must be non-empty) or `Esc` to cancel.

The request is added to:
- the collection root, if a collection or root request is selected
- the folder, if a folder or folder request is selected

**Loading a request (`Enter` on a request node)** — pressing `Enter` on a non-folder item loads the request into the Request tab and switches to it. Method, URL, headers, and body are all restored. The response area is cleared and the status bar confirms the load.

**Deleting (`d`)** — a confirmation modal shows the item name. Press `y` or `Enter` to confirm, `n` or `Esc` to cancel.

Method badges are colour-coded:

| Colour | Method |
|--------|--------|
| Green | GET |
| Blue | POST |
| Yellow | PUT |
| Magenta | PATCH |
| Red | DELETE |

### Env panel

Manage environment variables across multiple environments (Test, Staging, Production…).

The panel is split into two columns:

```
┌─ Environments ──────────┐  ┌─ Test — Variables ──────────────────────┐
│ ● Test                  │  │  API_URL              = https://test      │
│   Production            │  │  TOKEN                = secret-xxx        │
│   Staging               │  │  DEBUG                = true              │
└─────────────────────────┘  └─────────────────────────────────────────┘
```

`●` marks the **active environment** — the one whose variables will be injected into requests.

**Navigation:**
- `←` / `→` — switch focus between Environments (left) and Variables (right)
- `↑` / `↓` — navigate within the focused panel

**Editing:**

| Key | Action |
|-----|--------|
| `n` | Create a new environment |
| `a` | Add a variable to the selected environment |
| `d` | Delete the selected environment (focus left) or variable (focus right) |
| `Enter` | Activate the selected environment (focus left) |

**Creating an environment (`n`)** — prompts for a name. Saved to `<terapi_dir>/envs/<name>.toml`.

**Adding a variable (`a`)** — modal with two fields: Key and Value. Use `Tab` to switch between them. The variable is added to the currently selected environment. Variables are displayed sorted alphabetically.

**Activating an environment** — press `Enter` on an environment in the left panel. The `●` indicator moves to it. The active environment name is displayed in the Request panel URL bar title: ` URL · env: Test `. Its variables will be substituted in requests as `{{VAR}}` when request sending is implemented.

### History panel

Placeholder — will show recent requests in v0.4.

### Keybindings

| Key | Context | Action |
|-----|---------|--------|
| `Tab` | Global | Cycle panels: Request → Collections → History |
| `q` / `Esc` | Global | Quit |
| `e` | Request panel | Enter URL edit mode |
| `m` | Request panel | Cycle HTTP method (GET → POST → PUT → PATCH → DELETE) |
| `s` | Request panel | Send current request |
| `i` | Request panel (Body sub-tab) | Enter body editor mode |
| `←` / `→` | Request panel (response mode) | Navigate request sub-tabs |
| `←` / `→` | Request panel (URL mode) | Cycle HTTP method |
| `Enter` | Request panel (URL mode) | Send request |
| `Esc` | Request panel (URL mode) | Cancel URL edit |
| `Esc` | Request panel (body editor) | Exit body editor |
| `↑` / `↓` | Request panel | Move response cursor (JSON) / scroll (Raw) |
| `Enter` | Request panel (response mode) | Fold / unfold selected JSON node |
| `r` | Request panel | Toggle JSON ↔ Raw response view |
| `-` | Request panel | Shrink Key column |
| `=` | Request panel | Grow Key column |
| `↑` / `↓` | Collections panel | Move cursor |
| `Enter` | Collections panel (folder) | Expand / collapse folder |
| `Enter` | Collections panel (request) | Load request into Request tab |
| `n` | Collections panel | New collection |
| `f` | Collections panel | New folder in selected collection |
| `a` | Collections panel | Add request to selected collection / folder |
| `d` | Collections panel | Delete selected item |
| `←` / `→` | Env panel | Switch focus: Environments ↔ Variables |
| `↑` / `↓` | Env panel | Navigate within focused panel |
| `Enter` | Env panel (left) | Activate selected environment |
| `n` | Env panel | New environment |
| `a` | Env panel | Add variable to selected environment |
| `d` | Env panel | Delete selected environment or variable |
| `Tab` | Modal | Cycle input fields (Name ↔ URL, Key ↔ Value) |
| `←` / `→` | Modal (New Request) | Cycle HTTP method |
| `Enter` | Modal | Confirm |
| `Esc` | Modal | Cancel |

---

## Collections

Collections are TOML files that store groups of requests. They are loaded at TUI startup.

### Directory resolution

Terapi looks for collections in the first directory that matches, in order:

| Priority | Path | Typical use |
|----------|------|-------------|
| 1 | `$TERAPI_DIR/collections/` | Custom path, CI override |
| 2 | `./.terapi/collections/` | Per-project, committed to Git |
| 3 | `~/.config/terapi/collections/` | Global default |

**Per-project setup (recommended for teams):**

```bash
mkdir -p .terapi/collections
cp examples/collection.toml .terapi/collections/my-api.toml
# Edit, then optionally commit:
git add .terapi/
```

**CI override:**

```bash
TERAPI_DIR=./infra/terapi terapi run campaign.toml
```

### Collection TOML format

Each `.toml` file in the `collections/` directory represents one collection.

```toml
[collection]
name = "My API"
description = "Optional description"   # optional

# --- Folders (optional grouping) ---

[[folders]]
name = "Auth"

[[folders.requests]]
name = "Login"
method = "POST"
url = "https://api.example.com/auth/login"
description = "Obtain a JWT token"     # optional
body = '''
{
  "email": "{{EMAIL}}",
  "password": "{{PASSWORD}}"
}
'''

[folders.requests.headers]
Content-Type = "application/json"

[[folders.requests]]
name = "Refresh token"
method = "POST"
url = "https://api.example.com/auth/refresh"

[folders.requests.headers]
Authorization = "Bearer {{TOKEN}}"

# --- Root-level requests (no folder) ---

[[requests]]
name = "List users"
method = "GET"
url = "https://api.example.com/users"

[requests.headers]
Authorization = "Bearer {{TOKEN}}"

[[requests]]
name = "Create user"
method = "POST"
url = "https://api.example.com/users"
body = '{"name": "{{NAME}}", "email": "{{EMAIL}}"}'

[requests.headers]
Authorization = "Bearer {{TOKEN}}"
Content-Type = "application/json"
```

See `examples/collection.toml` for a fully annotated template.

---

## Demo mode

Load any JSON file directly into the response viewer without sending a real request:

```bash
terapi --demo response.json
terapi --demo demo.json        # bundled example
```

Useful for exploring the JSON viewer, testing fold behaviour, or demoing the TUI offline.

---

## Campaign runner

Run a sequence of HTTP requests headlessly from a TOML file:

```bash
terapi run campaign.toml
```

### Campaign TOML format

```toml
[campaign]
name        = "Users API — smoke tests"
description = "Login, then run CRUD operations"   # optional

# Load a named terapi environment as base vars (optional).
# Inline [env] overrides these; extracted step vars override everything.
env_file = "production"   # references <terapi_dir>/envs/production.toml

[env]
BASE_URL = "https://api.example.com"   # overrides env_file if same key
ADMIN    = "admin@example.com"

[[steps]]
name   = "Login"
method = "POST"
url    = "{{BASE_URL}}/auth/login"
body   = '{"email": "{{ADMIN}}", "password": "secret"}'

[steps.headers]
Content-Type = "application/json"

[steps.extract]
JWT     = "token"     # dot-path into the JSON response
USER_ID = "user.id"

[[steps]]
name   = "Get profile"
method = "GET"
url    = "{{BASE_URL}}/users/{{USER_ID}}"

[steps.headers]
Authorization = "Bearer {{JWT}}"

[[steps]]
name   = "Delete user"
method = "DELETE"
url    = "{{BASE_URL}}/users/{{USER_ID}}"

[steps.headers]
Authorization = "Bearer {{JWT}}"
```

### Variable substitution

`{{VAR}}` placeholders are replaced in `url`, `headers`, and `body` using values from (lowest to highest priority):

1. `env_file` — named terapi environment loaded from disk (campaign-level base)
2. `[env]` block — inline vars at campaign level, override `env_file`
3. Connector row variables — CSV columns, override campaign env
4. Step `env` — named terapi environment for that step only, overrides campaign base
5. `[steps.extract]` — values extracted from previous step responses (always highest priority)

**Per-step environment** — each step can declare `env = "name"` to use a specific terapi environment for that step. The step env overrides campaign-level vars, but extracted vars from previous steps always take precedence:

```toml
[[steps]]
name   = "Login (production)"
env    = "production"    # uses production.toml vars for this step
method = "POST"
url    = "{{BASE_URL}}/auth/login"

[[steps]]
name   = "Health check (staging)"
env    = "staging"       # uses staging.toml vars for this step
method = "GET"
url    = "{{BASE_URL}}/health"
```

### Variable extraction

Use dot-path notation in `[steps.extract]` to pull values out of a JSON response:

| Path | Extracts |
|------|----------|
| `token` | `response["token"]` |
| `user.id` | `response["user"]["id"]` |
| `data.items.0.name` | `response["data"]["items"][0]["name"]` |

Extracted values are injected into all subsequent steps.

### Data-driven campaigns (CSV)

Add a CSV connector to run the campaign once per row:

```toml
[[connectors]]
type = "csv"
path = "contacts.csv"   # relative to the campaign file

[[steps]]
name   = "Invite contact"
method = "POST"
url    = "{{BASE_URL}}/invitations"
body   = '{"email": "{{contact_email}}", "name": "{{contact_name}}"}'
```

CSV column names become `{{variables}}` automatically. See `examples/bulk_invite.toml` and `examples/contacts.csv`.

### Campaign output

```
Campaign : Users API — smoke tests

  ✓ Login                  POST    200    142 ms
      ↳ JWT = eyJhbGciOiJIUzI1NiIs…
      ↳ USER_ID = 42
  ✓ Get profile            GET     200     89 ms
  ✗ Delete user            DELETE  404     34 ms  HTTP 404

╔══════════════════════════════════════════════════════════════╗
║  Campaign Report — Users API — smoke tests                    ║
╠══════════════════════════════════════════════════════════════╣
║  Steps    : 2 ok  /  1 failed  (3 total)                     ║
║  Duration : 265 ms                                            ║
╠══════════════════════════════════════════════════════════════╣
║  ✗  SOME STEPS FAILED                                         ║
╚══════════════════════════════════════════════════════════════╝
```

Exit code is `0` if all steps pass, `1` if any step fails.

### Silent mode (CI/cron)

Suppress all output and return only the exit code:

```bash
terapi run campaign.toml --silent   # or -s
```

Useful in CI pipelines or cron jobs where logs are noisy.

```yaml
# GitHub Actions example
- name: API smoke tests
  run: terapi run infra/smoke.toml --silent
```
