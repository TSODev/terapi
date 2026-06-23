# Terapi — Usage Guide

## Table of contents

- [Installation](#installation)
- [TUI mode](#tui-mode)
  - [Panels](#panels)
  - [Request panel](#request-panel)
  - [Collections panel](#collections-panel)
  - [History panel](#history-panel)
  - [Campaigns panel](#campaigns-panel)
  - [GraphQL mode](#graphql-mode)
  - [Keybindings](#keybindings)
- [Collections](#collections)
  - [Directory resolution](#directory-resolution)
  - [Collection TOML format](#collection-toml-format)
- [Demo mode](#demo-mode)
- [Import](#import)
- [Campaign runner](#campaign-runner)
  - [Campaign TOML format](#campaign-toml-format)
  - [Campaign pipeline overview](#campaign-pipeline-overview)
  - [Campaign parameters](#campaign-parameters)
  - [Variable substitution](#variable-substitution)
  - [Variable extraction](#variable-extraction)
  - [Assertions](#assertions)
  - [Continue on error](#continue-on-error)
  - [Pause steps](#pause-steps)
  - [Transform steps](#transform-steps)
  - [Input connectors](#input-connectors)
    - [CSV connector](#csv-connector)
    - [JSON connector — from file](#json-connector--from-file)
    - [JSON connector — from seed step](#json-connector--from-seed-step)
  - [Output connectors](#output-connectors)
  - [Campaign examples](#campaign-examples)
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
| **Collections** | Browse saved collections and requests — default landing tab |
| **Request** | Build and send HTTP requests, view responses |
| **Env** | Create and manage environment variables across multiple environments |
| **History** | Persistent log of all sent requests and their responses |
| **Campaigns** | List, inspect, and run campaign TOML files with live step-by-step progress |

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
| Description | Free-text note about the request — `i` to edit, `Esc` to exit, persisted in collection TOML |
| Headers | Request headers — common header picker + custom entry |
| URL Params | Query string parameters |
| Body | Raw JSON body editor |
| Auth | Authentication — No Auth / Bearer / Basic / API Key |
| Options | TLS verification, timeout, redirects, cookie jar |

#### Auth sub-tab

Navigate to the Auth sub-tab with `←` / `→`. The sub-tab shows an interactive type selector and the fields for the selected type.

**Type selector** (always row 0):

```
 Type    No Auth    Bearer    Basic    API Key
```

The active type is highlighted in yellow. Press `Space` or `Enter` on this row to cycle through types.

**Bearer**

```
 Type    No Auth   ●Bearer●   Basic    API Key

 Token    eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9…
```

`Enter` on the Token row opens an edit modal. The token is injected as `Authorization: Bearer <token>` at send time.

**Basic**

```
 Type    No Auth    Bearer   ●Basic●   API Key

 Username  admin
 Password  ••••••••
```

Username and password are each editable in a modal. Password is always masked. At send time, `username:password` is Base64-encoded and sent as `Authorization: Basic …`.

**API Key**

```
 Type    No Auth    Bearer    Basic   ●API Key●

 Key Name   X-API-Key
 Key Value  sk-…
 Location   ●Header●   Query Param
```

`Enter` on Key Name / Key Value to edit. `Space` or `Enter` on the Location row toggles between **Header** (added as a request header) and **Query Param** (appended to the URL as `?<name>=<value>`).

In all modes, `{{VAR}}` placeholders in auth field values are resolved from the active environment at send time. Auth config is saved with the request when using `S` (Save to collection).

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

Navigate to the Options sub-tab with `←/→`. Use `↑`/`↓` to move between the four options, `Space` or `Enter` to toggle or cycle the selected one.

| Option | Default | Description |
|--------|---------|-------------|
| Skip TLS verification | off | Accept self-signed or hostname-mismatched certificates |
| Follow redirects | on | Automatically follow 3xx responses (up to 10 hops) |
| Timeout | 30 s | Request timeout — cycles through presets: 5 / 10 / 15 / 20 / 30 / 45 / 60 / 90 / 120 / 300 s |
| Cookie jar | off | Store received cookies and re-send them on subsequent requests (session mode) |

Active boolean options turn yellow. The timeout shows the current value in brackets (e.g. `[30s]`); each press of `Space`/`Enter` advances to the next preset, wrapping back to 5 s after 300 s.

```
┌─ Options ──────────────────────────────────────────────────────────┐
│                                                                      │
│▶ [ ] Skip TLS verification  (accept self-signed / mismatched certs) │
│                                                                      │
│  [x] Follow redirects        (automatically follow 3xx, up to 10)   │
│                                                                      │
│  [30s] Timeout               (Space/Enter cycles: 5→10→…→300 s)     │
│                                                                      │
│  [ ] Cookie jar              (store & send cookies across requests)  │
│                                                                      │
│  ↑/↓: navigate   Space/Enter: toggle / cycle timeout                │
└──────────────────────────────────────────────────────────────────────┘
```

**Cookie jar** — when enabled, terapi behaves like a browser for cookies: `Set-Cookie` headers received in responses are stored and automatically included in the `Cookie` header of subsequent requests. Useful for testing session-based authentication (login → session cookie → authenticated requests).

> **User-Agent** — terapi automatically sets `User-Agent: terapi/<version>` on every request. You can override it per-request by adding a `User-Agent` header in the Headers sub-tab.

The jar is cleared automatically when the option is toggled off or when starting a new request (`n`). All four options are persisted in the collection TOML and restored when loading a request from Collections.

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
- **Content-Type** opens a second picker with 9 common values (`application/json`, `multipart/form-data`, `text/plain`…); `Esc` goes back to the header picker
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

#### GraphQL mode

Press `g` on the Request tab to switch to **GraphQL mode**. The URL bar shows a magenta `GQL` badge instead of the method selector, and the sub-tabs switch to GraphQL-specific tabs. Press `g` again to return to REST mode (URL, headers, and auth are preserved).

**GraphQL sub-tabs** (navigate with `←`/`→`):

| Sub-tab | Purpose |
|---------|---------|
| Query | Multi-line editor — `i` to edit, `Esc` to exit; `{{VAR}}` picker; `Ctrl+Space` autocompletion |
| Variables | Key/value pairs serialised as the `variables` JSON object |
| Headers | Same header picker as REST mode (`a` add, `d` delete) |
| Schema | Schema browser — `f` fetch types, `↑/↓` navigate, `Enter` load fields |
| Options | Same options as REST mode (TLS, redirects, timeout, cookies) |

**Writing a query** (Query tab):
- Press `i` to enter the editor (border turns magenta)
- Full multi-line editing: arrows, Home/End, Backspace, Enter for new line
- Type `{{` to open the variable picker and insert `{{VAR_NAME}}` from the active environment
- Press `Ctrl+Space` to open the **autocompletion popup** (magenta border):
  - If a type detail is loaded from the Schema tab → lists its fields with their types
  - Otherwise → lists all OBJECT / INTERFACE / INPUT_OBJECT type names
  - `↑`/`↓` navigate, `Enter` or `Tab` inserts (replacing the prefix already typed), `Esc` closes
  - Typing filters in real time; no match closes the popup and passes the character through
- Press `Esc` to exit the editor

**Managing variables** (Variables tab):

| Key | Action |
|-----|--------|
| `a` | Add a variable (Key + Value modal, `Tab` switches fields) |
| `d` | Delete the selected variable |
| `Enter` | Edit the selected variable |
| `↑` / `↓` | Navigate variables |

Variables are serialised as a flat JSON object (`{"key": "value", …}`) and sent as the `variables` field alongside the query.

**Sending** — press `s` (or `Enter` in URL mode). Terapi builds `{"query": "...", "variables": {...}}` and posts it as JSON. `Content-Type: application/json` is added automatically if absent.

**Browsing the schema** (Schema tab):

1. Press `f` — sends `{ __schema { types { name kind } } }` and displays all user-defined types in the left panel with colour-coded kind badges:

   | Badge | Kind |
   |-------|------|
   | `OBJ` (cyan) | Object |
   | `ENM` (yellow) | Enum |
   | `INP` (green) | Input object |
   | `INT` (blue) | Interface |
   | `UNI` (magenta) | Union |

2. Navigate with `↑`/`↓`
3. Press `Enter` on a type — sends `{ __type(name: "X") { fields args enumValues } }` and displays fields, return types, and arg types in the right panel

Two-phase design (depth ≤ 3 per query) passes CDN depth limits enforced by proxies like Netlify GCDN.

**Saving to a collection** — press `S`. The TOML stores three extra fields:

```toml
graphql      = true
graphql_query = """
query FilmDetail($id: ID!) {
  film(filmID: $id) { title director }
}
"""
graphql_variables = {id = "ZmlsbXM6MQ=="}
```

Existing REST collections are unaffected (`#[serde(default)]`).

**Loading from Collections** — pressing `Enter` on a request node with `graphql = true` restores the query, variables, headers, and activates GraphQL mode automatically. The node displays a magenta `GQL` badge in the tree instead of the HTTP method.

**Response viewer** (bottom half of the Request panel):

The JSON view displays a 3-column table: **Key / Type / Value**.

- Objects and arrays show a `▼` / `▶` fold icon — press `Enter` to fold or unfold.
- Folded nodes display an inline content preview: `{ id: 42, name: "tsodev" … }`.
- Press `r` to cycle through three views: **JSON** → **Raw** → **HTTP** → JSON.
- Use `-` / `=` to shrink or grow the Key column width.
- Use `↑` / `↓` to move the cursor row by row (JSON view) or scroll (Raw / HTTP views).

**Extraction path bar:**

A line permanently displayed just below the JSON table shows the dot-notation path of the currently selected row:

```
┌─ JSON ────────────────────────────────────────────────┐
│  Key              Type     Value                       │
│  ▼ features       Array                               │
│    ▼ [0]          Object                              │
│      ▼ properties Object                              │
│▶       city       String   "Paris"                    │
│        zip        String   "75001"                    │
│        ...                                            │
├───────────────────────────────────────────────────────┤
│  ↳ features.0.properties.city                         │
└───────────────────────────────────────────────────────┘
```

The path shown (`features.0.properties.city`) is exactly the dot-path to paste into `[steps.extract]` in a campaign. See [Variable extraction](#variable-extraction).

**JSON search:**

Press `/` in the JSON view to open a search bar at the bottom. Type to filter — all rows whose **key** or **value** match are highlighted in yellow and bold. The cursor jumps to the first match automatically.

```
┌─ JSON ────────────────────────────────────────────────┐
│  ▼ (root)         Object                              │
│    id             Number   42                         │
│    **name**       String   **"Paris"**                │ ← highlighted
│    latitude       Number   48.8566                    │
│    **name**       String   **"Île-de-France"**        │ ← highlighted
│  ↳ features.0.properties.name                        │
├───────────────────────────────────────────────────────┤
│  / name█ 2 matches   >: next  <: prev  Esc: close    │
└───────────────────────────────────────────────────────┘
```

| Key | Action |
|-----|--------|
| `/` | Open search bar |
| type | Filter by key or value (case-insensitive) |
| `Backspace` | Delete last character |
| `>` | Jump to next match (wraps) |
| `<` | Jump to previous match (wraps) |
| `Esc` | Close search and clear filter |

**Response views:**

| View | Content |
|------|---------|
| JSON | Parsed JSON tree — foldable, colour-coded, cursor navigation, path bar, search |
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
| `e` | Edit the selected request (name, method, URL) |
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

**Editing a request (`e`)** — pressing `e` on a request node loads the request fully into the **Request tab** and switches to it. All fields are editable: URL (press `e` to enter URL mode), method (`m` or `↑`/`↓` in URL mode), headers, URL params, body, auth, and description (`i` to edit).

Press `S` to open the **Update Request** modal, pre-filled with the original name, collection, and folder:

| Action | Result |
|--------|--------|
| Keep name + keep location → `Enter` | Saves in place (overwrites) |
| Edit name + keep location → `Enter` | Renames the request in place |
| Change collection or folder → `Enter` | Saves as a new entry at the new location (original preserved) |
| `Esc` | Cancel — no changes written |

Press `n` to discard all edits and start a new blank request instead.

**Deleting (`d`)** — a confirmation modal shows the item name. Press `y` or `Enter` to confirm, `n` or `Esc` to cancel.

**Open in external editor (`E`)** — pressing `E` on any node in the tree (collection, folder, or request) opens the collection's TOML file in `$EDITOR` (fallback: `$VISUAL`, then `vi`). The TUI suspends, the editor takes the full terminal, and on exit terapi reloads all collections from disk. Any change made in the editor (rename, add a request, restructure folders) is immediately reflected in the TUI.

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

**Activating an environment** — press `Enter` on an environment in the left panel. The `●` indicator moves to it. The active environment name is displayed in the Request panel URL bar title: ` URL · env: Test `. Its variables are substituted in all `{{VAR}}` placeholders in the URL, headers, and body when a request is sent.

### History panel

Every request sent from the TUI is recorded automatically in `<terapi_dir>/history.toml` (newest first, max 100 entries). Both successful requests and transport errors are saved.

Each entry shows:
- **Timestamp** — UTC date and time (`YYYY-MM-DD HH:MM:SS`)
- **Mode** — `GQL` (magenta) for GraphQL requests, HTTP verb for REST
- **Status** — HTTP status code, colour-coded: green 2xx, yellow 3xx/4xx, red 5xx, grey for transport errors
- **Elapsed** — response time in ms (blank for errors)
- **URL** — the fully-resolved URL that was sent

Pressing `Enter` on an entry:
- **REST entry** — restores method, URL, headers, body; positions on Description sub-tab
- **GraphQL entry** — activates GraphQL mode, restores the query and variables; positions on the Query sub-tab

**Keybindings**

| Key | Action |
|-----|--------|
| `↑` / `↓` | Navigate entries |
| `Enter` | Load entry into the Request tab |
| `d` | Delete the selected entry (removed from list and saved to disk) |

### Campaigns panel

The Campaigns tab lists all `.toml` campaign files found in `<terapi_dir>/campaigns/` and lets you run them without leaving the TUI.

```
┌─ Campaigns (2) ──────────────────┐  ┌─ crud_demo ───────────────────────────────────────┐
│▶ crud_demo          (6 steps)    │  │  Name        JSONPlaceholder — CRUD Demo            │
│  transform_demo     (4 steps)    │  │  Description All HTTP methods with assertions       │
│                                  │  │                                                      │
│                                  │  │  Steps                                              │
│                                  │  │    POST   Create post                               │
│                                  │  │    GET    Read post                                 │
│                                  │  │    PUT    Update post                               │
│                                  │  │    PATCH  Patch post                                │
│                                  │  │    DELETE Delete post                               │
│                                  │  │    GET    Assert deleted                            │
│                                  │  │                                                      │
│                                  │  │  r to run this campaign                             │
└──────────────────────────────────┘  └──────────────────────────────────────────────────────┘
```

The **right panel** adapts to the run state:

| State | Content |
|-------|---------|
| **Idle** | Campaign metadata (name, description, step list with methods) and a `r` hint |
| **Running** | Completed steps appear one by one; `⟳ current step…` indicates what is in flight |
| **Done** | Colour-coded verdict, per-step status / timing / extracted vars / assertion failures |

**Keybindings:**

| Key | Action |
|-----|--------|
| `↑` / `↓` | Navigate campaign list |
| `r` | Run the selected campaign (opens params modal if `[[params]]` defined) |
| `E` | Open campaign TOML in `$EDITOR`, reload on exit |
| `Esc` | Clear run result (return to Idle) |

**Campaign parameters modal** — if the selected campaign declares `[[params]]`, pressing `r` opens a form instead of running immediately. Each parameter is shown with its current value (pre-filled from `default`):

```
┌──────────── Parameters — Itinéraire — Géoplateforme IGN ────────────────┐
│                                                                           │
│  DEPART               Paris                   Ville de départ            │
│▶ ARRIVEE              Lyon                    Ville d'arrivée            │
│  PROFILE              car                     car | pedestrian | cyclist  │
│  OPTIMIZATION         fastest                 fastest | shortest          │
│                                                                           │
│  Enter: edit value   r: run   Esc: cancel                                │
└───────────────────────────────────────────────────────────────────────────┘
```

| Key | Action |
|-----|--------|
| `↑` / `↓` | Navigate parameters |
| `Enter` | Edit the selected value (type, `Enter` to confirm, `Esc` to cancel) |
| `r` | Run the campaign with the current values |
| `Esc` | Close modal without running |

**Setting up campaigns:** place `.toml` files in `<terapi_dir>/campaigns/` (same priority resolution as collections). The quickest way is `terapi import`:

```bash
terapi import examples/crud_demo.toml
terapi import examples/transform_demo.toml
# or manually:
cp examples/crud_demo.toml ~/.config/terapi/campaigns/
```

### Context bar

A permanent two-line bar is always visible at the bottom of the screen:

```
Request  ›  Body  ›  JSON  ›  editing              ● env: Production
Tab: panels  e: edit URL  s: send  S: save  ←/→: section  q: quit
```

- **Top line** — breadcrumb of the current context (tab › sub-tab › mode › focus) on the left; active environment indicator on the right:
  - `● env: <name>` in green when an environment is active
  - `⚠ {{VAR}} not resolved` in yellow when on the Request tab, no env is active, and the request contains `{{VAR}}` placeholders — a reminder that variables will be sent literally
  - `○ no active env` in dim grey when none is selected and no unresolved variables are present
- **Bottom line** — contextual keybinding hints (change with every mode/tab switch)

### Keybindings

| Key | Context | Action |
|-----|---------|--------|
| `Tab` | Global | Cycle panels: Request → Collections → History |
| `q` | Global | Quit — press twice to confirm (status bar turns yellow on first press) |
| `Esc` | Global | Close modal / exit edit mode — does **not** quit the app |
| `n` | Request panel | New request — clear all fields |
| `e` | Request panel | Enter URL edit mode |
| `m` | Request panel | Cycle HTTP method (GET → POST → PUT → PATCH → DELETE) |
| `s` | Request panel | Send current request |
| `S` | Request panel | Save current request to a collection |
| `a` | Request panel (URL Params sub-tab) | Add param |
| `d` | Request panel (URL Params sub-tab) | Delete selected param |
| `Enter` | Request panel (URL Params sub-tab) | Edit selected param |
| `↑` / `↓` | Request panel (URL Params sub-tab) | Navigate params |
| `↑` / `↓` | Request panel (Auth sub-tab) | Navigate between auth fields |
| `Space` / `Enter` | Request panel (Auth sub-tab, Type row) | Cycle auth type |
| `Enter` | Request panel (Auth sub-tab, field row) | Open edit modal for field value |
| `Space` | Request panel (Auth sub-tab, Location row) | Toggle API Key location: Header ↔ Query Param |
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
| `/` | Request panel (JSON view) | Open search bar — filter rows by key or value |
| `>` | JSON search | Jump to next match |
| `<` | JSON search | Jump to previous match |
| `Esc` | JSON search | Close search and clear filter |
| `-` | Request panel | Shrink Key column |
| `=` | Request panel | Grow Key column |
| `↑` / `↓` | Collections panel | Move cursor |
| `Enter` | Collections panel (folder) | Expand / collapse folder |
| `Enter` | Collections panel (request) | Load request into Request tab |
| `n` | Collections panel | New collection |
| `f` | Collections panel | New folder in selected collection |
| `a` | Collections panel | Add request to selected collection / folder |
| `e` | Collections panel (request) | Edit request (name, method, URL) |
| `E` | Collections panel | Open collection TOML in `$EDITOR`, reload on exit |
| `d` | Collections panel | Delete selected item |
| `←` / `→` | Env panel | Switch focus: Environments ↔ Variables |
| `↑` / `↓` | Env panel | Navigate within focused panel |
| `Enter` | Env panel (left) | Activate selected environment |
| `n` | Env panel | New environment |
| `a` | Env panel | Add variable to selected environment |
| `d` | Env panel | Delete selected environment or variable |
| `↑` / `↓` | History panel | Navigate entries |
| `Enter` | History panel | Load entry into Request tab |
| `d` | History panel | Delete selected entry |
| `↑` / `↓` | Campaigns panel | Navigate campaign list |
| `r` | Campaigns panel | Run campaign (or open params modal if `[[params]]` defined) |
| `E` | Campaigns panel | Open campaign TOML in `$EDITOR`, reload on exit |
| `Esc` | Campaigns panel | Clear run result |
| `↑` / `↓` | Campaign params modal | Navigate parameters |
| `Enter` | Campaign params modal | Edit selected value |
| `r` | Campaign params modal | Run with current values |
| `Esc` | Campaign params modal | Cancel (close without running) |
| `g` | Request panel | Toggle GraphQL mode (REST ↔ GraphQL) |
| `i` | GraphQL Query tab | Enter query editor |
| `Ctrl+Space` | GraphQL Query editor | Open autocompletion popup |
| `Esc` | GraphQL Query editor | Exit editor |
| `a` | GraphQL Variables tab | Add variable |
| `d` | GraphQL Variables tab | Delete selected variable |
| `Enter` | GraphQL Variables tab | Edit selected variable |
| `←` / `→` | GraphQL mode | Navigate GraphQL sub-tabs |
| `f` | GraphQL Schema tab | Fetch type list via introspection |
| `↑` / `↓` | GraphQL Schema tab | Navigate type list |
| `Enter` | GraphQL Schema tab | Load fields for selected type |
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

**GraphQL request fields** — add these to any `[[requests]]` or `[[folders.requests]]` block:

```toml
[[folders.requests]]
name         = "Tous les pays"
method       = "POST"
url          = "https://countries.trevorblades.com/graphql"
graphql      = true
graphql_query = """
{
  countries {
    code
    name
    capital
    emoji
  }
}
"""
```

Variables are stored as an inline table:

```toml
[[folders.requests]]
name         = "Détail d'un pays"
method       = "POST"
url          = "https://countries.trevorblades.com/graphql"
graphql      = true
graphql_query = """
query CountryDetail($code: ID!) {
  country(code: $code) {
    name  capital  currency
    continent { name }
  }
}
"""
graphql_variables = {code = "FR"}
```

At send time terapi builds `{"query": "...", "variables": {"code": "FR"}}` and injects `Content-Type: application/json`. `{{VAR}}` placeholders in the query and variable values are resolved from the active environment.

### Collections d'exemple

Des collections prêtes à l'emploi sont disponibles dans `examples/collections/` :

| Fichier | Contenu | Dossiers | Requêtes | Auth |
|---------|---------|----------|----------|------|
| `public-rest.toml` | JSONPlaceholder, ReqRes, httpbin, PokeAPI, CoinGecko | 5 | ~30 | Aucune |
| `graphql.toml` | Countries API, Rick & Morty (POST GraphQL) | 2 | ~10 | Aucune |
| `rick-morty-graphql.toml` | Rick & Morty API — personnages, épisodes, lieux, filtres, pagination, introspection | 6 | 17 | Aucune |
| `countries-graphql.toml` | Countries API — pays, continents, langues, filtres, introspection | 5 | 19 | Aucune |
| `sncf.toml` | SNCF — gares, horaires, itinéraires, perturbations | 6 | 20 | Basic `{{SNCF_TOKEN}}` |
| `france-geo.toml` | API Géo + IGN — communes, départements, régions, géocodage | 4 | 19 | Aucune |
| `france-eau.toml` | Hub'Eau — hydrométrie, qualité rivières et nappes | 3 | 19 | Aucune |
| `france-meteo.toml` | Météo-France — prévisions, observations, vigilance | 4 | 17 | Bearer `{{METEO_TOKEN}}` |

**Installation rapide :**

```bash
# Global (~/.config/terapi/collections/)
cp examples/collections/france-geo.toml ~/.config/terapi/collections/

# Projet local (.terapi/collections/)
mkdir -p .terapi/collections
cp examples/collections/sncf.toml .terapi/collections/
```

Pour les collections avec authentification, créez un environnement dans l'onglet **Env** et ajoutez la variable correspondante (`SNCF_TOKEN` ou `METEO_TOKEN`), puis activez-le avec `Enter`.

---

## Demo mode

Load any JSON file directly into the response viewer without sending a real request:

```bash
terapi --demo response.json
terapi --demo demo.json        # bundled example
```

Useful for exploring the JSON viewer, testing fold behaviour, or demoing the TUI offline.

---

## Import

`terapi import` accepts both **collection** and **campaign** TOML files. It auto-detects the type from the TOML content (`[collection]` vs `[campaign]`) and copies the file to the correct sub-directory:

| TOML section | Destination |
|---|---|
| `[collection]` | `<terapi_dir>/collections/` |
| `[campaign]` | `<terapi_dir>/campaigns/` |

```bash
# Import a collection
terapi import examples/collections/france-geo.toml

# Import a campaign
terapi import examples/crud_demo.toml

# Import everything at once
for f in examples/collections/*.toml examples/*.toml; do terapi import "$f"; done
```

The destination filename is derived from the `name` field in `[collection]` or `[campaign]`. If a file already exists it is overwritten and reported as `Updated`.

**Output:**

```
Imported collection "France — Géographie" → /Users/you/.config/terapi/collections/france-géographie.toml
Updated  collection "France — Géographie" → /Users/you/.config/terapi/collections/france-géographie.toml
Imported campaign  "JSONPlaceholder — CRUD Demo" → /Users/you/.config/terapi/campaigns/jsonplaceholder-crud-demo.toml
```

Files with neither `[collection]` nor `[campaign]` produce a clear error. The directory resolution follows the same priority as the TUI: `$TERAPI_DIR` → `./.terapi/` → `~/.config/terapi/`.

For collections that require authentication (`sncf.toml`, `france-meteo.toml`), open the **Env** tab, create an environment, add the required variable (`SNCF_TOKEN` or `METEO_TOKEN`), and activate it with `Enter`.

---

## Campaign runner

Campaigns can be run in two ways:

- **TUI** — open the **Campaigns** tab, select a campaign, press `r` (see [Campaigns panel](#campaigns-panel))
- **CLI headless** — `terapi run campaign.toml` (ideal for CI/cron)

```bash
terapi run campaign.toml

# Override declared [[params]] at run time
terapi run campaign.toml -p DEPART=Bordeaux -p ARRIVEE=Nantes

# Multiple -p flags are cumulative; unset params fall back to their default
terapi run campaign.toml -p ENV=staging -p TIMEOUT=60
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

### Campaign pipeline overview

A campaign is a directed pipeline. Data flows from left to right — each stage's output feeds the next:

```
┌──────────────────────────────────────────────────────────────────────────┐
│                       CAMPAIGN PIPELINE                                   │
│                                                                            │
│  ┌─────────────────┐                                                       │
│  │  env_file / [env]│ ──────────────────────────────────────────────────┐ │
│  └─────────────────┘    base variables (lowest priority)                │ │
│                                                                          ↓ │
│  ┌───────────────────────────────────────────────────────────────┐      │ │
│  │                    [[connectors]]                              │      │ │
│  │                                                                │      │ │
│  │  type = "csv"   →  one row per CSV line                       │      │ │
│  │  type = "json"  →  one row per JSON array element             │  ────┼─┤ │
│  │    from file                                                   │      │ │
│  │    from seed step  (kind = "seed" HTTP step, run once first)  │      │ │
│  │                                                                │      │ │
│  │  (no connector) →  single run, no row variables               │      │ │
│  └───────────────────────────────────────────────────────────────┘      │ │
│                          │  row variables (override env)                 │ │
│                          ↓                                               │ │
│  ┌────────────────────────────────────────────────────────────────┐     │ │
│  │  for each row:   [[steps]]                                      │ ←──┘ │
│  │                                                                  │      │
│  │  kind = "http" (default)                                         │      │
│  │    → resolve {{VAR}}, send request                               │      │
│  │    → assert response (optional)                                  │      │
│  │    → [steps.extract]  →  new {{VARS}} for next steps             │      │
│  │                                                                  │      │
│  │  kind = "seed"                                                   │      │
│  │    → run once before iteration, feeds [[connectors]]             │      │
│  │                                                                  │      │
│  │  kind = "transform"                                              │      │
│  │    → reshape/compute variables without HTTP                      │      │
│  └──────────────────────────────────────────────────────────────┬─┘      │
│                                                                  │         │
│                          extracted {{VARS}}                      │         │
│                          (highest priority)                      ↓         │
│                                                ┌──────────────────────┐   │
│                                                │    [[outputs]]       │   │
│                                                │  write JSON to disk  │   │
│                                                └──────────────────────┘   │
└──────────────────────────────────────────────────────────────────────────┘
```

**Variable priority** (lowest → highest, each level overrides the previous):

| Priority | Source |
|----------|--------|
| 1 | `env_file` — named terapi environment loaded from disk |
| 2 | `[env]` — inline block in the campaign TOML |
| 3 | `[[params]]` defaults — user-facing inputs, override `[env]` if key not already set |
| 4 | Connector row — CSV columns or JSON object fields |
| 5 | Step `env` — named environment applied to one step only |
| 6 | `[steps.extract]` — values extracted from previous step responses |
| 7 | Runtime overrides — `-p KEY=VALUE` (CLI) or params modal (TUI) — highest priority |

---

### Campaign parameters

`[[params]]` declares user-facing inputs that can be overridden at run time — by `-p` on the CLI or the TUI params modal. Internal/technical variables belong in `[env]`.

```toml
[[params]]
name        = "DEPART"
description = "Ville de départ"   # shown in CLI header and TUI modal
default     = "Paris"             # used when no override is provided

[[params]]
name        = "ARRIVEE"
description = "Ville d'arrivée"
default     = "Lyon"

[[params]]
name        = "PROFILE"
description = "car | pedestrian | cyclist"
default     = "car"

[env]
# Internal variables — not intended to be overridden
RESOURCE    = "bdtopo-valhalla"
GEOCODE_URL = "https://data.geopf.fr/geocodage/search"
```

**Fields:**

| Field | Required | Description |
|-------|----------|-------------|
| `name` | yes | Variable name, used as `{{NAME}}` in steps |
| `description` | no | Human-readable hint shown in CLI output and TUI modal |
| `default` | no | Value used when no override is provided; omit to make the param required |

**CLI override:**

```bash
terapi run campaign.toml -p DEPART=Bordeaux -p ARRIVEE=Nantes -p PROFILE=pedestrian
```

Params not overridden fall back to their `default`. The CLI header always shows each param with its effective value:

```
Campaign : Itinéraire — Géoplateforme IGN
Params   :
  DEPART       = Bordeaux  (Ville de départ)
  ARRIVEE      = Nantes    (Ville d'arrivée)
  PROFILE      = car       (car | pedestrian | cyclist)
  OPTIMIZATION = fastest   (fastest | shortest)
```

**TUI override:** pressing `r` on a campaign with `[[params]]` opens the params modal. Edit values interactively, then press `r` to run.

---

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

> **Tip — find the right path in the TUI:** send the request in the Request panel, navigate to the key you want in the JSON view with `↑`/`↓`, and read the dot-path shown in the `↳` bar at the bottom of the response. That string is the exact value to use in `[steps.extract]`.
>
> ```toml
> [steps.extract]
> CITY = "features.0.properties.city"   # ← copied from the ↳ bar
> ```

### Assertions

Add `assert = [...]` to any step to validate the response. All assertions are evaluated; if any fails the step is marked `✗` and the campaign stops. Extracted vars are only propagated on full success.

```toml
[[steps]]
name   = "Login"
method = "POST"
url    = "{{BASE_URL}}/auth/login"
body   = '{"email": "{{ADMIN}}", "password": "secret"}'

[steps.headers]
Content-Type = "application/json"

assert = [
  { on = "status",              eq      = 200            },
  { on = "body.user.active",    eq      = true            },
  { on = "body.token",          exists  = true            },
  { on = "elapsed_ms",          lt      = 500             },
  { on = "header.content-type", contains = "json"         },
]

[steps.extract]
TOKEN   = "token"
USER_ID = "user.id"
```

**`on` — what to assert against:**

| Value | Targets |
|-------|---------|
| `"status"` | HTTP status code (number) |
| `"elapsed_ms"` | Response time in milliseconds (number) |
| `"body"` | Full parsed JSON body |
| `"body.x.y"` | Dot-path inside the JSON body |
| `"header.x-name"` | Response header value (case-insensitive) |

**Operators:**

| Operator | Description | Example |
|----------|-------------|---------|
| `eq` | Strict equality — string, number, or bool | `{ on = "status", eq = 201 }` |
| `ne` | Not equal | `{ on = "body.error", ne = true }` |
| `lt` / `lte` | Less than / less than or equal (numeric) | `{ on = "elapsed_ms", lt = 500 }` |
| `gt` / `gte` | Greater than / greater than or equal (numeric) | `{ on = "body.count", gt = 0 }` |
| `in` | Value is in allowed list | `{ on = "status", in = [200, 201] }` |
| `exists` | Field is present and non-null | `{ on = "body.token", exists = true }` |
| `contains` | String contains substring | `{ on = "header.content-type", contains = "json" }` |
| `matches` | String matches regex | `{ on = "header.location", matches = "/users/\\d+" }` |

`{{VAR}}` placeholders are resolved in `on`, `eq`, `contains`, and `matches` before comparison. String `"42"` and number `42` are considered equal by `eq`.

**Output when assertions fail:**

```
  ✗ Login             POST    200    684 ms  2 assertion(s) failed
      ✗ assert: body.user.active == true  (got false)
      ✗ assert: elapsed_ms < 500  (got 684)
```

Assertion failures also appear in the boxed report under the failed step.

### Continue on error

By default a failing step stops the pipeline immediately. Set `continue_on_error = true` to let the campaign run all steps regardless of individual failures.

**Campaign-level default** — applies to every step that does not override it:

```toml
continue_on_error = true   # all steps are non-blocking by default

[campaign]
name = "Full smoke suite"
```

**Step-level override** — takes priority over the campaign default:

```toml
continue_on_error = true        # non-blocking by default

[campaign]
name = "Mixed suite"

[[steps]]
name   = "Login (must succeed)"
method = "POST"
url    = "{{BASE_URL}}/auth/login"
continue_on_error = false       # this step is blocking: failure stops everything

[steps.extract]
JWT = "token"

[[steps]]
name              = "Optional analytics check"
method            = "GET"
url               = "{{BASE_URL}}/analytics"
continue_on_error = true        # redundant here (campaign default), shown for clarity
assert            = [{ on = "status", eq = 200 }]

[[steps]]
name   = "List users (always runs)"
method = "GET"
url    = "{{BASE_URL}}/users"

[steps.headers]
Authorization = "Bearer {{JWT}}"
```

**Rules:**

| Situation | Behaviour |
|-----------|-----------|
| Step succeeds | Variables extracted, next step runs |
| Step fails + `continue_on_error = true` | Marked `✗`, variables **not** extracted, next step runs |
| Step fails + `continue_on_error = false` | Marked `✗`, pipeline stops (default) |
| Step-level value | Overrides campaign-level for that step |
| Exit code | `1` if **any** step failed, even non-blocking ones |

**CLI output:**

```
  ✓ Login (must succeed)   POST   201    210 ms
  ✗ Optional analytics     GET    503     87 ms  HTTP 503  [continu]
      ✗ assert: status == 200  (got 503)
  ✓ List users (always runs) GET  200     91 ms
```

`[continu]` flags a non-blocking failure in the CLI output. In the TUI Campaigns panel the same step shows `[↷]` in grey.

The boxed report still lists all failures — `continue_on_error` only controls flow, not visibility.

### Pause steps

A `kind = "pause"` step waits for a fixed duration without making any HTTP request. Use it between steps to respect API rate limits.

```toml
[[steps]]
name    = "Rate limit pause"
kind    = "pause"
wait_ms = 1000   # wait 1 000 ms (1 second) before the next step
```

The step appears as `WAIT` in the CLI output and the TUI Campaigns panel, with the actual elapsed time shown:

```
  ✓ Rate limit pause    WAIT    -     1002 ms
```

`continue_on_error` applies to pause steps like any other: if set to `true` at campaign level, a hypothetical failure (impossible in practice) would be non-blocking.

---

### Transform steps

A `kind = "transform"` step processes variables without making an HTTP request. Use it to reshape data between steps — regex extraction from a header, string composition, case normalization, etc.

```toml
[[steps]]
name   = "Extract user ID from Location header"
kind   = "transform"
transforms = [
  { type = "regex",    input = "{{LOCATION}}", pattern = "/users/(\\d+)", group = 1, output = "USER_ID" },
  { type = "template", input = "Hello {{FIRST}} {{LAST}}",                           output = "GREETING" },
  { type = "upper",    input = "{{USERNAME}}",                                        output = "USERNAME_UPPER" },
]
```

Transforms within a step **chain** — each transform sees the outputs of previous ones in the same step.

**`type` — available operations:**

| Type | What it does | Extra fields |
|------|-------------|--------------|
| `template` | Resolve `{{VAR}}` in `input`, copy to `output` | — |
| `regex` | Extract capture group from `input` | `pattern` (required), `group` (default `1`) |
| `replace` | Replace `from` with `to` in `input` | `from` (required), `to` (default `""`) |
| `split` | Split `input` by `delimiter`, take element at `index` | `delimiter` (default `","`) , `index` (default `0`) |
| `trim` | Strip leading/trailing whitespace | — |
| `upper` | Convert to uppercase | — |
| `lower` | Convert to lowercase | — |

**Examples:**

```toml
# Extract JWT from "Bearer eyJ..." header value
{ type = "regex",   input = "{{AUTH_HEADER}}", pattern = "Bearer (.+)", group = 1, output = "TOKEN" }

# Take the first element of a comma-separated list
{ type = "split",   input = "{{CSV_IDS}}", delimiter = ",", index = 0, output = "FIRST_ID" }

# Compose a full name from two variables
{ type = "template", input = "{{FIRST}} {{LAST}}", output = "FULL_NAME" }

# Strip whitespace returned by a sloppy API
{ type = "trim",    input = "{{DIRTY_VALUE}}", output = "CLEAN_VALUE" }
```

Transform steps appear as `TRSF` in the campaign output, with extracted variables shown as `↳ VAR = value` like any other step.

### Input connectors

A `[[connectors]]` block tells the campaign how to build its iteration set. Without one, the campaign runs exactly once. With one, it runs once per row in the data source.

All connector variables merge with the campaign `[env]` (connector row overrides same-named env vars). Extracted vars from steps always win regardless.

---

#### CSV connector

Iterate over a CSV file — one run per row. Column headers become `{{variable}}` names.

```toml
[[connectors]]
type = "csv"
path = "contacts.csv"   # path relative to where terapi is run

[[steps]]
name   = "Invite contact"
method = "POST"
url    = "{{BASE_URL}}/invitations"
body   = '{"email": "{{contact_email}}", "name": "{{contact_name}}"}'
```

`contacts.csv`:
```
contact_email,contact_name
alice@example.com,Alice
bob@example.com,Bob
```

- Column names map directly to `{{variable}}` names (case-sensitive)
- Leading/trailing whitespace is trimmed from both keys and values
- All values are strings — use a `transform` step to cast if needed

See `examples/bulk_invite.toml` and `examples/contacts.csv`.

---

#### JSON connector — from file

Iterate over a JSON file — one run per element of a JSON array. Object fields at each element level become `{{variable}}` names (nested objects are flattened with dot-notation; arrays are serialised as JSON strings).

```toml
[[connectors]]
type   = "json"
path   = "users.json"
select = "users"          # dot-path to the array; omit or leave empty for root

[[steps]]
name   = "Get user posts"
method = "GET"
url    = "{{BASE_URL}}/posts?userId={{id}}"
```

`users.json`:
```json
{
  "users": [
    { "id": 1, "name": "Alice" },
    { "id": 2, "name": "Bob" }
  ]
}
```

**`select`** (optional) — dot-path to the target array inside the JSON. If the root of the file is already an array, omit `select` or set it to `""`.

| `select` | Array targeted |
|----------|---------------|
| *(omitted)* | Root — file must be a JSON array |
| `""` | Root (same as omitted) |
| `"users"` | `json["users"]` |
| `"data.items"` | `json["data"]["items"]` |

**Flattening rules:**

| JSON type | Variable value |
|-----------|---------------|
| string | raw string value |
| number | string representation |
| boolean | `"true"` / `"false"` |
| null | empty string `""` |
| object | flattened recursively: `parent.child` |
| array | serialised as JSON string `[...]` |

Example: `{ "address": { "city": "Paris", "zip": "75001" } }` produces `{{address.city}} = Paris` and `{{address.zip}} = 75001`.

See `examples/json_connector_demo.toml` and `examples/users.json`.

---

#### JSON connector — from seed step

Use the JSON response of an HTTP step as the data source — no file required. The seed step runs **once** before the iteration loop, its response body is parsed as JSON, and the resulting rows feed into the repeating steps.

```toml
[[connectors]]
type      = "json"
from_step = "Fetch cities"    # name of the seed step
select    = ""                # dot-path into the response (empty = root array)

[[steps]]
name   = "Fetch cities"
kind   = "seed"               # run once, not iterated
method = "GET"
url    = "https://geo.api.gouv.fr/communes?nom=Bordeaux&fields=nom,code"

[[steps]]
name   = "City detail"
method = "GET"
url    = "https://geo.api.gouv.fr/communes/{{code}}?fields=nom,code,population"

assert = [{ on = "status", eq = 200 }]

[steps.extract]
nom_commune = "nom"
population  = "population"
```

**How it works:**
1. The seed step (`kind = "seed"`) is executed once with the base environment
2. Its JSON response body is parsed using the `select` dot-path (same rules as the file connector)
3. Each element of the resulting array becomes one iteration row
4. The seed step does **not** appear in the iteration loop — it is transparent to the step runner

**Rules:**
- `from_step` must match the `name` of exactly one step with `kind = "seed"` in the same campaign
- Only one connector per campaign is currently supported
- The seed step's response must be valid JSON; otherwise the campaign aborts with an error
- `select = ""` selects the root of the response (if the response is directly a JSON array)
- `select = "data.items"` navigates into a nested array the same way as the file connector

See `examples/seed_step_demo.toml` for a complete working example.

---

### Output connectors

After all iterations complete, `[[outputs]]` blocks write step results to disk as JSON files. Use this to archive responses, pass data between campaigns, or build lightweight ETL pipelines.

```toml
[[outputs]]
from_step = "City detail"        # name of the step whose body to collect
path      = "/tmp/cities.json"   # output file path (parent dirs created if needed)
select    = ""                   # optional: dot-path to extract a sub-field
```

**What gets written:**

A single JSON file containing an **array** — one element per successful iteration of the named step:

```json
[
  { "nom": "Bordeaux", "code": "33063", "population": 267991 },
  { "nom": "Bordeaux-Saint-Clair", "code": "76117", "population": 658 },
  ...
]
```

- Failed iterations (HTTP error or assertion failure) are **skipped** — the array contains only successful results
- Output is pretty-printed JSON (indented)
- If the named step produced no successful results, a warning is emitted and no file is written

**`select`** (optional) — extract a sub-field from each response body before writing:

```toml
[[outputs]]
from_step = "Search results"
path      = "/tmp/names.json"
select    = "results.items"     # write only json["results"]["items"] for each iteration
```

**Chaining campaigns** — the output of one campaign feeds into the next via the JSON file connector:

```toml
# campaign_a.toml
[[outputs]]
from_step = "Fetch users"
path      = "/tmp/users.json"

# campaign_b.toml
[[connectors]]
type   = "json"
path   = "/tmp/users.json"
select = ""
```

```bash
terapi run campaign_a.toml && terapi run campaign_b.toml
```

**Multiple outputs** — multiple `[[outputs]]` blocks are supported, each naming a different step:

```toml
[[outputs]]
from_step = "Login"
path      = "/tmp/tokens.json"

[[outputs]]
from_step = "Get profile"
path      = "/tmp/profiles.json"
select    = "user"            # save only the "user" sub-object from each response
```

The CLI confirms each file written at the end of the report:

```
╚════════════════════╝
  → output written: /tmp/cities.json
  → output written: /tmp/profiles.json
```

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

### Campaign examples

Ready-to-run campaigns in `examples/` — no API key required:

| File | API | What it demonstrates |
|------|-----|----------------------|
| `crud_demo.toml` | JSONPlaceholder | All HTTP methods (GET/POST/PUT/PATCH/DELETE) with assertions on status, body fields, and elapsed time |
| `debug_toolbox.toml` | httpbin.io | Query param echo, header inspection, bearer auth check — assertions on nested body fields |
| `transform_demo.toml` | JSONPlaceholder | Transform steps: regex email parsing, uppercase, template composition, chained transforms |
| `auth_flow.toml` | ReqRes | Login → token extraction → authenticated request (requires a free ReqRes API key) |
| `bulk_invite.toml` | *(mock)* | CSV connector: one campaign iteration per CSV row |
| `json_connector_demo.toml` | JSONPlaceholder | JSON file connector: iterate over `examples/users.json`, fetch posts for each user |
| `seed_step_demo.toml` | API Géo (France) | Seed step + JSON connector + output connector: fetch a city list, iterate for details, write to `/tmp/communes_bordeaux.json` |
| `itineraire_demo.toml` | IGN Géoplateforme | **`[[params]]` + full pipeline**: geocode two cities, compose coordinates, compute road itinerary — no API key required |

```bash
terapi run examples/crud_demo.toml
terapi run examples/debug_toolbox.toml
terapi run examples/transform_demo.toml
terapi run examples/json_connector_demo.toml
terapi run examples/seed_step_demo.toml

# itineraire_demo uses [[params]] — run with defaults or override:
terapi run examples/itineraire_demo.toml
terapi run examples/itineraire_demo.toml -p DEPART=Bordeaux -p ARRIVEE=Nantes
terapi run examples/itineraire_demo.toml -p DEPART=Marseille -p ARRIVEE=Strasbourg -p PROFILE=car
```

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
