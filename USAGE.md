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

**Workflow — building a request from scratch:**
1. Press `n` to clear all fields and start a new blank request
2. Press `e` to enter URL edit mode — type the URL, use `↑`/`↓` to set the method, `Enter` to send or `Esc` to cancel
3. Navigate sub-tabs (`←`/`→`) to add headers, URL params, and body
4. Press `s` to send at any time
5. Press `S` to save the current request to a collection (see below)

**Sending a request:**
1. Press `e` to enter URL edit mode
2. Type the URL (Backspace to delete)
3. Use `↑` / `↓` to change the HTTP method
4. Press `←` / `→` to exit URL mode and jump directly to a sub-tab (Headers, Body…)
5. Press `Enter` to send — or `Esc` to finish editing without sending
6. Alternatively, press `s` at any time to send the current URL without entering edit mode

`{{VAR}}` placeholders in the URL (and all other fields) are automatically resolved from the active environment before the request is sent.

#### Variable auto-completion (`{{`)

Typing `{{` in any editable field opens a picker overlay showing the variables available in the active environment:

```
┌─ Insert variable · filter: TO ──────────┐
│  {{TOKEN}}  = eyJhbGciOiJIUzI...         │
│▶ {{TOKEN_EXP}}  = 3600                   │
│                                          │
│  ↑/↓: navigate  Enter: insert  Esc: cancel │
└──────────────────────────────────────────┘
```

- Works in: URL bar, header values, URL param values, body JSON field values, body text
- Continue typing after `{{` to filter the list in real time
- `Enter` inserts the selected variable as `{{VAR_NAME}}`
- `Esc` closes the picker and leaves `{{` as typed
- `Backspace` with an empty filter removes one `{` and closes the picker
- If no environment is active, a message in the status bar reminds you to activate one in the Env tab

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
| Headers | Request headers — common header picker + custom entry |
| URL Params | Query string parameters |
| Body | Raw JSON body editor |
| Auth | Bearer token, API Key, OAuth2 |
| Options | TLS verification, timeout, redirects |

#### Saving a request (`S`)

Press `S` (Shift+s) from anywhere in the Request tab to save the current request state to a collection:

```
┌──────────────── Save Request ───────────────────┐
│                                                  │
│  Name:        Get Pikachu_                       │
│                                                  │
│  Collection:  ↑ Public REST APIs ↓  (1/2)       │
│                                                  │
│  Folder:      ↑ PokeAPI ↓          (3/6)        │
│                                                  │
│  Tab: next field   ↑/↓: navigate   Enter: save  │
└──────────────────────────────────────────────────┘
```

- **Name** — free text, required
- **Collection** — `↑`/`↓` to cycle through collections; counter shows position
- **Folder** — `↑`/`↓` to cycle through folders in the selected collection plus `(root)` for the collection root
- `Tab` moves between the three fields
- `Enter` saves and writes to disk immediately; the request appears in the Collections tab
- `Esc` cancels without saving

The saved request includes method, URL (with query params appended), headers, and body.

#### Options sub-tab

Navigate to the Options sub-tab with `←/→` to configure per-request options:

| Option | Default | Description |
|--------|---------|-------------|
| Skip TLS verification | off | Accept self-signed or hostname-mismatched certificates |

Press `Space` or `Enter` to toggle the highlighted option. When **Skip TLS verification** is active the checkbox turns yellow — a visual reminder that cert validation is disabled.

```
┌─ Options ───────────────────────────────────────────────────────┐
│                                                                   │
│  [x] Skip TLS verification  (accept self-signed / mismatched…)   │
│                                                                   │
│      Space or Enter to toggle                                     │
└───────────────────────────────────────────────────────────────────┘
```

Useful for local dev servers, VPS endpoints with self-signed certs, or APIs behind a corporate proxy.

#### URL Params editor

Switch to the URL Params sub-tab and use the same keys as the headers editor, with the addition of `Enter` to edit a selected param:

```
┌─ URL Params (2) ────────────────────────────────────────────────┐
│  page                         = 2                                │
│▶ limit                        = 10                               │
└──────────────────────────────────────────────────────────────────┘
```

#### Headers editor

Press `a` to add a header. A picker appears with the most common HTTP headers:

```
┌─ Add header ──────────────────────────────────────────┐
│  Authorization         Bearer ...                      │
│▶ Content-Type          application/json                │
│  Accept                application/json                │
│  Accept-Language       en-US,en;q=0.9                 │
│  Accept-Encoding       gzip, deflate, br               │
│  Cache-Control         no-cache                        │
│  X-API-Key                                             │
│  X-Request-ID                                          │
│  User-Agent                                            │
│  Origin                                                │
│  Referer                                               │
│  Custom…                                               │
│  ↑/↓: navigate  Enter: select  Esc: cancel            │
└───────────────────────────────────────────────────────┘
```

- Selecting a common header pre-fills the key and default value; the modal opens with the cursor on the **value** field, ready to edit
- **Custom…** opens a blank modal with the cursor on the **key** field
- `{{` in the value field opens the variable picker (active env required)

| Key | Action |
|-----|--------|
| `a` | Add a param (Key + Value modal, `Tab` to switch fields) |
| `d` | Delete selected param |
| `Enter` | Edit selected param |
| `↑` / `↓` | Navigate params |

At send time params are recomposed as a query string and appended to the URL (`?key=value&key2=value2`). If the URL already contains a `?`, params are joined with `&`.

**Auto-parse on load** — when a request is loaded from Collections and its URL contains a query string (e.g. `https://api.example.com/users?page=2&limit=10`), terapi splits it automatically: the URL bar receives the base URL (`https://api.example.com/users`) and the params list is populated with the parsed key/value pairs.

#### Body editor

The body editor has two modes, toggled with `t` (when the Body sub-tab is active and outside edit mode).

**Text mode** (default)

```
┌─ Body  [Text]  (4 lines)  i: edit  t: JSON mode ───────────────┐
│ {                                                                 │
│   "email": "admin@example.com",                                  │
│   "password": "{{PASSWORD}}"                                     │
│ }                                                                 │
└──────────────────────────────────────────────────────────────────┘
```

Press `i` to enter edit mode (border turns green). Full multi-line editing: arrows, Home/End, Backspace/Delete. Press `Esc` to exit.

**JSON mode** (structured key/value)

```
┌─ Body  [JSON]  (2 fields)  i: edit  t: text mode ──────────────┐
│  Key                Value                                         │
│  email              "admin@example.com"                          │
│▶ password           "{{PASSWORD}}"                               │
└──────────────────────────────────────────────────────────────────┘
```

Press `i` to enter the field editor (border turns green), then:

| Key | Action |
|-----|--------|
| `a` | Add a new field (Key + Value modal) |
| `d` | Delete the selected field |
| `Enter` / `e` | Edit the selected field |
| `↑` / `↓` | Navigate fields |
| `Esc` | Exit field editor |

Values are **auto-typed** when the request is sent:

| Value | Serialized as |
|-------|---------------|
| `42`, `-3`, `1.5` | JSON number |
| `true` / `false` | JSON boolean |
| `null` | JSON null |
| anything else | JSON string (with quotes) |

**Switching modes** — pressing `t` converts content between modes:
- **Text → JSON**: the textarea is parsed as a JSON object; if valid, fields are extracted into the table
- **JSON → Text**: fields are serialized back to pretty-printed JSON in the textarea

An empty body (no text / no fields) sends no request body.

**Response viewer** (bottom half of the Request panel):

The JSON view displays a 3-column table: **Key / Type / Value**.

- Objects and arrays show a `▼` / `▶` fold icon — press `Enter` to fold or unfold.
- Folded nodes display an inline content preview: `{ id: 42, name: "tsodev" … }`.
- Press `r` to cycle through three views: **JSON** → **Raw** → **HTTP** → JSON.
- Use `-` / `=` to shrink or grow the Key column width.
- Use `↑` / `↓` to move the cursor row by row (JSON view) or scroll (Raw / HTTP views).

**Response views:**

| View | Content |
|------|---------|
| JSON | Parsed JSON tree — foldable, colour-coded, cursor navigation |
| Raw | Plain response body text |
| HTTP | Full HTTP exchange: request line + headers + body, then response status + headers + body |

The **HTTP view** is especially useful for debugging — it shows the exact request that was sent (with all `{{VAR}}` already resolved) and the full response headers:

```
── Request ──────────────────────────────────────────────
POST /login HTTP/1.1
Host: api.tsodev.fr
Content-Type: application/json
Content-Length: 45

{"username":"thierry","password":"Pr0bleme#"}

── Response ─────────────────────────────────────────────
HTTP/1.1 200 OK
Content-Type: application/json
Date: Tue, 02 Jun 2026 08:34:44 GMT
...

{"token":"eyJ0eXAiOiJKV1Qi…"}
```

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

### Context bar

A permanent two-line bar is always visible at the bottom of the screen:

```
Request  ›  Body  ›  JSON  ›  editing              ● env: Production
Tab: panels  e: edit URL  s: send  S: save  ←/→: section  q: quit
```

- **Top line** — breadcrumb of the current context (tab › sub-tab › mode › focus) on the left; active environment indicator on the right:
  - `● env: <name>` in green when an environment is active
  - `○ no active env` in dim grey when none is selected
- **Bottom line** — contextual keybinding hints (change with every mode/tab switch)

### Keybindings

| Key | Context | Action |
|-----|---------|--------|
| `Tab` | Global | Cycle panels: Request → Collections → History |
| `q` / `Esc` | Global | Quit |
| `n` | Request panel | New request — clear all fields |
| `e` | Request panel | Enter URL edit mode |
| `m` | Request panel | Cycle HTTP method (GET → POST → PUT → PATCH → DELETE) |
| `s` | Request panel | Send current request |
| `S` | Request panel | Save current request to a collection |
| `a` | Request panel (URL Params sub-tab) | Add param |
| `d` | Request panel (URL Params sub-tab) | Delete selected param |
| `Enter` | Request panel (URL Params sub-tab) | Edit selected param |
| `↑` / `↓` | Request panel (URL Params sub-tab) | Navigate params |
| `i` | Request panel (Body sub-tab) | Enter body editor mode |
| `t` | Request panel (Body sub-tab, outside editor) | Toggle body mode: Text ↔ JSON |
| `a` | Body editor (JSON mode) | Add field |
| `d` | Body editor (JSON mode) | Delete selected field |
| `Enter` / `e` | Body editor (JSON mode) | Edit selected field |
| `↑` / `↓` | Body editor (JSON mode) | Navigate fields |
| `←` / `→` | Request panel (response mode) | Navigate request sub-tabs |
| `←` / `→` | Request panel (URL mode) | Navigate sub-tabs (exit URL mode) |
| `↑` / `↓` | Request panel (URL mode) | Cycle HTTP method |
| `Enter` | Request panel (URL mode) | Send request |
| `Esc` | Request panel (URL mode) | Finish URL edit (stay on current sub-tab) |
| `Esc` | Request panel (body editor, Text or JSON) | Exit body editor |
| `↑` / `↓` | Request panel | Move response cursor (JSON) / scroll (Raw) |
| `Enter` | Request panel (response mode) | Fold / unfold selected JSON node |
| `r` | Request panel | Cycle response view: JSON → Raw → HTTP exchange |
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
