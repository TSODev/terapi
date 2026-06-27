# Changelog

All notable changes to this project will be documented in this file.
Format follows [Keep a Changelog](https://keepachangelog.com/en/1.0.0/).

---

## [Unreleased]

### Added
- `kind = "poll"` campaign step — poll an HTTP endpoint until an `until` condition is met or timeout expires
  - `until = { var, eq?, ne?, exists?, lt?, lte? }` — same operators as `when`, evaluated on extracted vars after each poll
  - `interval_ms` (default 1000, min 100) — delay between polls; `timeout_secs` (default 60) — max wait
  - Safety cap of 500 iterations regardless of timeout
  - TUI status bar shows `⟳ poll #N — step name — Ns`; badge `POLL` (yellow) in pipeline and CLI output
  - Full step editor in `terapi build`: URL, method, headers, extract, until condition, interval, timeout, continue_on_error
- `kind = "set"` campaign step — assign literal/template variables without HTTP
  - `[steps.vars]` key/value table; all values support `{{VAR}}` interpolation
  - Badge `SET` (blue) in pipeline and CLI output
  - Full step editor in `terapi build`: vars list with add/edit/delete
- `kind = "jq"` campaign step — apply a jq filter expression to a JSON variable using the system `jq` binary
  - `jq_input` (JSON variable), `jq_expression` (jq filter), `jq_output` (default `JQ_RESULT`), `jq_raw` (bool, passes `-r`)
  - Fails immediately with a clear error if `jq` is not found on the system
  - Badge `JQ` (green) in pipeline and CLI output
  - Full step editor in `terapi build`

- `kind = "parallel"` campaign step — run multiple named steps concurrently, wait for all to complete
  - `steps = ["Step A", "Step B"]` — named steps are pre-scanned and skipped in the sequential flow
  - Extractions from all children are merged (last-write-wins on conflict)
  - `continue_on_error = true` makes the parallel step succeed even if some children fail
  - Badge `PAR` (cyan) in pipeline and CLI output
  - Full step editor in `terapi build`: Steps list with `a`/`d` to add/remove names
- `kind = "notify"` campaign step — POST a webhook message (Slack, Discord, Teams, custom)
  - `url` + `message` (supports `{{VAR}}`) + optional `headers` + `method` (default POST)
  - `Content-Type: application/json` injected automatically unless overridden
  - Supports `when` for conditional notification
  - Badge `NTFY` (magenta) in pipeline and CLI output
  - Full step editor in `terapi build`

### Changed
- `jq` availability is now checked explicitly before spawning the process; missing binary produces a user-friendly error instead of an OS error code
- Campaign Builder: step editor now shows a contextual **Help** strip at the bottom of the editor panel when a step is selected — 3-line description (what it does · key behavior · keybindings), adapts to each step type (`http`, `seed`, `transform`, `pause`, `file`, `search`, `jq`, `poll`, `set`, `loop`, `parallel`, `notify`, `comment`)
- Campaign Builder: pipeline panel and all context panels now have 1 line of top padding inside the border for visual breathing room
- Campaign Builder: step run result panel now shows the **full JSON body** (no truncation) with syntax highlighting (keys=cyan, strings=green, numbers=yellow, booleans=magenta); scrollable with `[` / `]` (or `Fn+↑` / `Fn+↓` on Mac)
- Campaign Builder: **Parallel step picker** — adding a step to a parallel's list now opens a visual picker overlay (↑/↓ + Enter) instead of a free-text prompt; only `http`, `graphql`, `seed`, `poll`, `loop` steps are listed (steps that perform network requests)
- Campaign Builder ergonomics — **all list sections** (Headers, Extractions, Assertions, GQL Variables, Multipart Parts, Transforms, Parallel Steps, Set Vars) now support:
  - `↑`/`↓` navigates items within the section before jumping to the next section
  - `d` deletes **at the cursor** (not always the last item)
  - `Enter` opens the item **under the cursor** for editing (Headers, GQL Variables, Loop/Poll Headers, Multipart Parts, Transforms)
- Campaign Builder: **`transform` step supports multiple transforms** — the Transforms section is now a navigable list; `a` adds a new transform, `Enter` edits the selected one (kind ←/→ cycle, then input var, then output var via Tab/Enter flow), `d` deletes at cursor; replaces the old single-transform Kind/Input/Output fields
- Campaign Builder: `AddPairStage2` hint (value field) now shows `Tab: JSON path picker` when the target is an Extract field
- Campaign Builder: **Assertions now support `Enter` to edit** — `Enter` on an existing assertion pre-fills path, operator (pre-selected to current op), and value; `Enter` saves by replacing the assertion at that position (not pushing a new one); hints show `(edit mode)` to distinguish from add flow

---

## [0.9.3] — 2026-06-26

### Changed
- Updated crates.io description to better reflect current scope

---

## [0.9.2] — 2026-06-26

### Added
- `terapi run --only <step-name>` (repeatable) — run only the named step(s), skip all others; skipped steps appear as `⊘ (skipped)` without failing the pipeline; vars from skipped steps are absent (same semantics as `when = false`)
- `terapi run --format json` — emit a single JSON object on stdout: `campaign`, `success`, `duration_ms`, `steps[]` (or `iterations[]` for connector campaigns); each step includes `name`, `method`, `url`, `status`, `success`, `skipped`, `elapsed_ms`, `extracted`, `assertions`, `error`
- `terapi run --format csv` — emit one CSV row per step: `iteration,name,method,url,status,success,skipped,elapsed_ms,extracted,error`; `extracted` is JSON-encoded in the cell; safe quoting (RFC 4180)
- `--only` and `--format` are combinable: `terapi run campaign.toml --only Login --format json`
- `terapi run --retry N` — retry failed HTTP/GraphQL/seed steps up to N times with exponential backoff (`min(2^(attempt-1), 30)` seconds between attempts); transform/pause/file/search/comment/loop steps are not retried; text output shows `⟳ retry K/N — <step> — waiting Xs...` on stderr

### Fixed
- Import Postman v2.1 — `urlencoded` body now properly percent-encoded (RFC 1866) and `Content-Type: application/x-www-form-urlencoded` injected automatically; no longer reported as a degradation

---

## [0.9.1] — 2026-06-26

### Added
- `kind = "search"` campaign step — filter a JSON array by regex on a field
  - `search = { input = "{{VAR}}", path = "field.path", match = "regex", output = "OUT" }`
  - `path = ""` matches directly on string elements (no field navigation)
  - `first_only = true` stores first matching element as object; default stores all matches as array
  - Returns `"null"` / `[]` when no match
  - Badge `SRCH` (cyan) in TUI and builder pipeline
  - Full step editor in `terapi build`: Input var, Match-on field, Pattern, Output var, First match toggle
  - TOML serialization in builder with regex escaping

---

## [0.9.0] — 2026-06-26

### Added
- `terapi import <file.json>` — import Postman v2.1 and Insomnia v4 collections and environments
  - Auto-detects collection vs environment from JSON structure
  - Imports folders (one level; nested sub-folders flattened with "Sub / Request" naming)
  - Imports requests: method, URL (raw with query string), headers, body (raw/GraphQL/urlencoded/formdata)
  - Imports auth: Bearer, Basic, API Key, OAuth2 (mapped to Client Credentials)
  - Collection-level auth inherited by requests with no explicit auth
  - GraphQL body (`mode: "graphql"`) → GQL mode with query + variables
  - Collection variables → terapi env named `"<collection> vars"`
  - Postman environments (JSON with `_postman_variable_scope`) → terapi env
  - Disabled headers, params, and env values are skipped
  - Import report: counts (requests, folders, env vars), warnings (scripts ignored, formdata/urlencoded degraded), destination path
- `terapi import <file.json>` — import Insomnia v4 exports
  - Auto-detected via `_type: "export"` + `resources` array
  - Workspace → collection; request_groups → folders; requests → StoredRequest
  - gRPC and WebSocket requests skipped with warning count
  - Sub-environments merged with base environment vars; each saved as a separate terapi env
  - Auth: Bearer, Basic, API Key, OAuth2 CC and AC (detected from grant_type)
  - GraphQL body (`mimeType: "application/graphql"`) → GQL mode

---

## [0.8.4] — 2026-06-26

### Changed
- README: replace ASCII diagram with real GraphQL screenshot (hero + GraphQL section)

---

## [0.8.3] — 2026-06-26

### Added

- **`kind = "loop"` step (pagination)** — new step type in the catalog (badge: `LOOP`, green). Repeats an HTTP request until an `until` condition is met, accumulating values from each response into a campaign variable.
  - `until = {var, eq?, ne?, exists?, lt?, lte?}` — stop condition evaluated after each iteration (reuses `StepCondition`, extended with `lt`/`lte` for numeric page/total comparisons)
  - `accumulate = {var, from}` — dot-path extraction (supports `*` wildcard) run on each response; results appended to a JSON array stored in `var`
  - Safety cap: 1000 iterations max
  - Campaign runner: `run_loop_step()` in `campaign.rs` — per-iteration env update + until check; accumulated array JSON-encoded into extracted vars
  - Builder: full step editor with sections `URL`, `Method`, `Headers`, `Until — var`, `Until — condition` (cycle with `Enter`/`←`/`→`), `Accumulate — var`, `Accumulate — from`, `Extract (per-iter)`, `Continue on error`; `←`/`→` on `Until — condition` cycles `not exists → exists → == → != → <`
  - TOML preview and save include `kind`, `until` (inline table), `accumulate` (inline table)
  - Step summary in pipeline view: `<url> → <acc_var> until <until_var>`

---

## [0.8.2] — 2026-06-26

### Added

- **GraphQL step in builder** (`kind = "graphql"`) — new brick type in the catalog. Fields: URL, GraphQL query (multi-line textarea, `i`/`Esc`), variables (key/value list), headers, assertions, `when`, `foreach`, `continue_on_error`. Badge: `GQL` (magenta). TOML preview and save include `graphql_query` (literal block string) and `[steps.graphql_variables]`. Query and variable values are fully resolved before execution. Supported in `Checker` (undefined `{{VAR}}` references scanned).
- **Extract item editing in builder** — pressing `Enter` on an extract entry opens edit mode with the value pre-filled; `←`/`→`/`Home`/`End`/`Delete` for cursor navigation within the value; `Enter` to confirm. Hint line updated: `a: add  d: del  Enter: edit  ↑↓: navigate`.
- **Extract item navigation** — `↑`/`↓` navigate between extract entries (cyan `▶` cursor); `d` deletes the entry under the cursor (not always the last one).
- **Cursor navigation in all text fields** — `←`/`→` move the insertion cursor within any text field in the step editor (`EditText` mode); `Home`/`End` jump to start/end; `Delete` removes the character under the cursor.

### Fixed

- **TOML preview missing extract / headers / body** — the TOML preview (`p`) used a separate `generate_toml_preview` function that omitted `body`, `headers`, `extract`, `transforms`, `graphql_query`, `graphql_variables`, and file fields. Preview now delegates to the same `generate_toml` used by the save command (`w`).
- **TOML field ordering** — `when`, `assert`, and `transforms` were serialized after `[steps.extract]`, placing them inside the extract subtable (TOML spec violation). `[steps.graphql_variables]` was also emitted before scalar fields. All inline scalars (`when`/`assert`/`transforms`/`foreach`/`continue_on_error`/etc.) now appear before any `[subtable]` headers.
- **Run step (`r`) ignores extracted variables from preceding steps** — the single-step preview only received base env (`env_file` + campaign `[env]`). Preceding steps are now executed in sequence to accumulate their extracted variables before the target step runs, so `{{VAR}}` references produced by earlier steps resolve correctly.
- **`L` (load from collection) not working** — the shortcut only triggered when the cursor was on the `LoadFromCollection` section. Now available globally from any section in HTTP, GraphQL, and Seed step editors.
- **Builder secondary text readability** — replaced `Indexed(242)` (near-invisible on black) with `Indexed(246)` throughout the builder UI; `Indexed(238)` hints raised to `Indexed(242)`; separator lines raised from `Indexed(236)` to `Indexed(240)`; `DarkGray` elements raised to `Indexed(244)`.

---

## [0.8.1] — 2026-06-25

### Changed
- README: Campaign Builder section moved to the end (after OAuth2), before Stack.

### Fixed
- `examples/campaigns/crud_demo.toml`: removed stray comment.

---

## [0.8.0] — 2026-06-25 — Campaign Builder (`terapi build`)

### Added

- **`terapi build`** — new interactive TUI for authoring and editing campaign TOML files without leaving the terminal. `terapi build` opens a blank campaign; `terapi build <file.toml>` edits an existing one.

- **Pipeline view** — left panel (40%) lists all steps in order with numbered badges (`HTTP` / `TRSF` / `WAIT` / `SEED` / `FILE` / `#`). Secondary lines below each step show `foreach` target, `when` condition, and assertion count. Navigation wraps through `[IN]` (connectors above step 0) and `[OUT]` (outputs below the last step).

- **Brick catalog** — 8 step types selectable with `i` when browsing the pipeline:
  - `HTTP` — URL, method, headers, body, assertions, extract, when, foreach
  - `Transform` — template / regex / replace / split / trim / upper / lower
  - `Pause` — `wait_ms` delay
  - `Seed` — HTTP step whose response feeds a connector
  - `Comment` — annotation-only, never executed
  - `File Loader` — read a file and encode it as base64 / text / hex
  - `Connector [IN]` — CSV or JSON data source; navigable as `[IN]` node
  - `Output [OUT]` — write collected step responses to a JSON file; navigable as `[OUT]` node

- **Step editor** — right panel (60%) shows all fields for the focused step type. Field sections: URL/method, headers (two-stage add), body (multi-line `tui-textarea`, yellow border, `Esc` to save), extract (key→dot-path pairs), assertions, `when` condition, `foreach`, `continue_on_error`. `Tab` / `Shift+Tab` cycle sections; `Enter` edits the focused field.

- **`kind = "file"` — File Loader step** — reads a file from disk and stores its content in a campaign variable. Three encodings: `base64` (default), `text`, `hex`. `file_output` defaults to `FILE_DATA`. Cycles with `Space` in the encoding field. Badge: `FILE` (magenta).

- **`[[steps.multipart_parts]]` — multipart form-data** — HTTP steps can declare a list of form parts (instead of `body`). Each part has `name`, `value`, and optional `content_type`. Prefix value with `@` to load a file as binary. `{{VAR}}` is resolved in both `name` and `value`.

- **Collection browser** — `b` opens the full collection tree. Navigate with `↑`/`↓`, expand with `Enter`, load into the current step with `l` (populates method, URL, headers, body). Exit with `Esc`.

- **Variables panel** (`v`) — full CRUD on the campaign `[env]` block: browse with `↑`/`↓`, add with `a`, edit (rename + value, `Tab` switches fields) with `Enter`, delete with `d`.

- **Connectors editor** — dedicated editor for `[[connectors]]` entries (CSV / JSON). Reachable via the `[IN]` node in the pipeline. `a` add, `d` delete, `Enter` edit fields.

- **Outputs editor** — dedicated editor for `[[outputs]]` entries. Reachable via the `[OUT]` node. `a` add, `d` delete; `from_step` field opens a step picker filtered to HTTP and Seed steps only.

- **Campaign settings** (`s`) — edit campaign-level metadata: name, description, `continue_on_error`, `env_file`, and `[[params]]` entries.

- **Run step** (`r` in Browse mode) — executes the currently focused HTTP/Seed step in isolation (merging campaign `[env]` + `env_file` variables). The right panel splits 55/45: step editor above, result preview below. Preview shows: colour-coded status code, elapsed time, resolved URL, transport error (if any), assertion results (`✓`/`✗`), extracted variable values, and the first 6 lines of the response body.

- **JSON path autocomplete** — when editing an Extract value field, `Tab` opens an `ExtractPicker` overlay (magenta border). Paths are generated from the last run-step result: object keys, array indices (first 10), and `array.*.field` wildcard patterns. Type to filter; `↑`/`↓` navigate; `Enter` inserts the selected path; `Esc`/`Tab` return to the field editor.

- **Checker** (`c`) — static pipeline validation with 10+ rules (colour-coded `OK` / `⚠ Warning` / `✗ Error`):
  - Undefined `{{VAR}}` references (URL, headers, body, foreach, when, multipart values)
  - Undefined `foreach` source variable
  - Empty step names; duplicate step names
  - File Loader: empty `file_path`
  - HTTP steps: empty URL; multipart parts with empty name
  - Transform steps: no transforms defined
  - Output `from_step`: empty or no matching step name; empty `path`
  - Connector `from_step`: set but no matching step name; path empty with no `from_step`

- **TOML preview** (`p`) — shows the generated TOML for the current campaign with full syntax highlighting: `[[array.sections]]` → magenta bold; `[sections]` → cyan bold; string values → green; numbers and booleans → yellow; multi-line `'''…'''` blocks → green.

- **Quit confirmation** — pressing `q` when the campaign has unsaved changes (`modified = true`) shows a centered overlay: `Save before quitting? [y] save & quit  [n] discard  [Esc] cancel`.

- **Step operations** — `K`/`J` move the focused step up/down in the pipeline; `x` deletes; `i` inserts a new step from the catalog after the cursor.

- **Save** (`w`) — writes the campaign TOML to its original path (when editing an existing file) or to `<terapi_dir>/campaigns/` (when building from scratch). Body fields serialized as TOML literal strings (`'...'` / `'''...'''`); transforms as inline table arrays.

- **Example campaign** — `examples/campaigns/upload_demo.toml` — 5-step demo using postman-echo.com: File Loader (base64) → File Loader (text) → POST base64 in JSON body → POST multipart text parts → POST multipart `@file` binary part.

### Changed

- `campaign.rs` — `run_single_step` (private) is now exposed as `pub async fn run_step_preview(step, env) -> StepResult`, a thin public wrapper used by the builder's run-step feature.

### Docs

- `USAGE.md` — new Campaign Builder section (ASCII layout, catalog reference, all keybindings, step editor fields by type, checker rules table); new File Loader and multipart form-data sections.
- `BUILDER.md` — content merged into `USAGE.md` and file removed from the repository.
- `README.md` — replaced "Coming Soon — Campaign Builder" placeholder with shipped feature description; added `terapi build` to the usage block.

---

## [0.7.8] — 2026-06-24

### Added
- **Édition de variable d'environnement** — dans l'onglet Env, `Enter` sur une variable du panneau droit (Variables) ouvre un modal d'édition pré-rempli avec la clé et la valeur actuelles. La clé est entièrement modifiable (renommage) ; `Tab` bascule entre les deux champs ; `Enter` sauvegarde. Si la clé est renommée, l'ancienne entrée est supprimée et la nouvelle est insérée. L'environnement est persisté immédiatement sur disque.

- **Pré-remplissage intelligent du modal Save Request** — lors du chargement d'une requête depuis le panel Collections (via `Enter` ou `e`), puis à chaque sauvegarde réussie dans la session, le modal `S` (Save/Update Request) s'ouvre pré-rempli avec le nom, la collection et le dossier d'origine. Cela évite de ressaisir ces informations lors d'une re-sauvegarde après modification.

- **Création inline de collection dans le modal Save** — dans le modal Save Request, lorsque le focus est sur le champ Collection, appuyer sur `n` ouvre un champ de saisie inline. Taper le nom et valider crée la collection et la sélectionne automatiquement, sans quitter le modal.

- **Création inline de dossier dans le modal Save** — dans le modal Save Request, lorsque le focus est sur le champ Folder, appuyer sur `n` ouvre un champ de saisie inline. Taper le nom et valider crée le dossier dans la collection courante et le sélectionne automatiquement.

- **`Tab` atteint toujours le champ Folder** — dans le modal Save Request, `Tab` depuis le champ Collection bascule systématiquement vers Folder, même si aucun dossier n'existe encore dans la collection. Cela permet de créer un dossier (`n`) sans workaround.

- **Duplication de requête (`D`)** — dans le panel Collections, appuyer sur `D` sur une requête charge une copie de celle-ci dans l'onglet Request (tous les champs : URL, méthode, headers, body, auth, description, variables GraphQL) et ouvre directement le modal Save Request pré-rempli avec le nom `<nom> copy`, sans origine définie, prêt à être sauvegardé sous un nouveau nom ou dans un autre dossier/collection.

- **Tri alphabétique dans le panel Collections** — les collections, les dossiers dans chaque collection, et les requêtes dans chaque dossier ou à la racine sont désormais affichés par ordre alphabétique croissant (insensible à la casse). Le tri est appliqué au niveau de l'affichage (`flatten_stored` / `flatten_stored_full`) sans modifier l'ordre des données sous-jacentes, ce qui préserve la validité des `NodeAddress`.

---

## [0.7.7] — 2026-06-24

### Added
- **`{{item_0}}`, `{{item_1}}`, … dans les steps `foreach`** — quand un élément d'un tableau `foreach` est lui-même un tableau JSON (ex. `[lon, lat]`), terapi injecte automatiquement des variables `item_0`, `item_1`, etc. dans l'environnement d'itération. De même, si l'élément est un objet JSON, les champs sont accessibles via `item_nomduchampe`. Cela permet d'itérer sur des tableaux de tableaux (ex. coordonnées GPS) sans étape de transformation intermédiaire.
- **Campagne `itineraire_demo.toml` étendue** — la campagne de démonstration IGN Géoplateforme inclut désormais une étape de géocodage inverse : elle extrait les 35 points de départ des étapes de route (`portions.0.steps.*.geometry.coordinates.0`), appelle l'API reverse-geocoding pour chacun (`{{item_0}}` = lon, `{{item_1}}` = lat), et produit un fichier JSON `itineraire_etapes.json` avec : ville de départ, ville d'arrivée, distance, durée et liste des adresses de passage.

---

## [0.7.6] — 2026-06-24

### Added
- **Recherche / filtre dans le panel Collections** — appuyer sur `/` dans l'onglet Collections ouvre une barre de recherche en bas du panel. La saisie filtre l'arbre en temps réel : seuls les nœuds correspondants (et leurs parents en grisé pour le contexte) sont affichés, avec le fragment correspondant mis en évidence en jaune. `↑`/`↓` naviguent dans la liste filtrée ; `Enter` charge directement la requête dans l'onglet Request ; `Esc` ferme la barre et restaure l'arbre complet. La recherche parcourt tout l'arbre, y compris les dossiers repliés.

---

## [0.7.5] — 2026-06-24

### Added
- **Shift+Tab** — navigue les onglets principaux dans le sens inverse (Collections ← Request ← Env ← History ← Campaigns).

- **Charger un step de campagne dans le Request tab (`L`)** — dans le panel Done de l'onglet Campaigns (focus Result), `↑`/`↓` déplace un curseur `▶` (cyan) entre les steps HTTP. Appuyer sur `L` charge le step sélectionné dans l'onglet Request avec tous les champs résolus (URL, méthode, headers, body — les `{{VAR}}` sont déjà substitués) puis bascule sur cet onglet. Permet de rejouer le step (`s`), de l'inspecter en vue HTTP (`r` deux fois), de modifier les headers, ou de le sauvegarder dans une collection (`S`). Les steps WAIT et TRSF sont ignorés par le curseur. `StepResult` stocke désormais un snapshot `request_headers` + `request_body` capturé au moment de l'exécution.

- **`when` — exécution conditionnelle de step** — tout step accepte désormais un champ `when` (table TOML inline) qui évalue une variable de campagne avant d'exécuter le step. Si la condition est fausse, le step est ignoré (`⊘ skipped`) sans interrompre la campagne ni compter comme échec. Opérateurs supportés :
  - `eq = "valeur"` — la variable est égale à la valeur
  - `ne = "valeur"` — la variable est différente de la valeur
  - `exists = true/false` — la variable est (ou n'est pas) définie dans l'environnement
  - *(sans opérateur)* — la variable existe et est non vide

  La valeur de comparaison supporte `{{VAR}}` pour comparer deux variables. Le champ `var` désigne une variable de campagne (extraite d'un step précédent, de l'env ou du CSV).

  Exemple TOML :
  ```toml
  extract = { USER_TYPE = "user.type" }

  [[steps]]
  name = "Premium flow"
  when = { var = "USER_TYPE", eq = "premium" }
  method = "POST"
  url = "{{BASE}}/premium/activate"
  ```

  Affichage TUI : dans la vue idle, chaque step avec `when` affiche `⊘ if VAR == "valeur"` en gris sous le nom du step (comme les hints `?` d'assertions). Dans les vues Running/Done, les steps ignorés affichent `⊘ (skipped)` en gris.

### Fixed
- **Suppression de collection non persistée** — `delete_collection()` reconstruisait le chemin du fichier depuis le nom de la collection via `sanitize_filename()` (ex. `"Public GraphQL APIs"` → `public-graphql-apis.toml`), ce qui échouait silencieusement quand le fichier avait été importé sous un nom différent (ex. `02-graphql.toml`). La suppression utilisait désormais `StoredCollection.path`, le chemin réel du fichier rempli à la lecture.

---

## [0.7.2] — 2026-06-24 — Redirect chain & cookie jar visibility

### Added
- **Redirect chain capture** — terapi now handles redirects manually (instead of delegating to reqwest's auto-follow). Each 3xx hop is recorded with its status code and resolved destination URL. The HTTP view shows a new `── Redirects ──` section listing every hop (e.g. `1  301 → https://www.example.com/`) with colour-coded status codes (301/308 yellow, 302/303 cyan, 307 blue). Up to 20 hops are captured.

- **Cookie jar visibility in HTTP view** — `Set-Cookie` response headers are now parsed into a structured `response_cookies` list on `App`. Two new sections appear in the HTTP view:
  - **Request section** — when the cookie jar is enabled, a reconstructed `Cookie: name=value; …` header line shows what cookies would be sent in the next request (drawn from the cookies received in the last response).
  - **`── Cookies ──` section** — after the response body, each received `Set-Cookie` is displayed as `name=value` (yellow) followed by its attributes (Path, Secure, HttpOnly…) in grey. Useful to understand session and tracking cookies without reading raw headers.

- **URL resolution for relative redirects** — `Location: /new-path` is correctly resolved against the current URL base (scheme + host + port) using `reqwest::Url::join`.

### Changed
- `execute_http` in `app/http.rs` now takes a `follow_redirects: bool` parameter. When `true`, it loops over 3xx responses and builds the `redirect_chain`. Schema introspection calls (`fetch_schema`, `fetch_type_detail`) pass `false` — they never need to follow redirects.

---

## [0.7.1] — 2026-06-24 — foreach, wildcard extraction, JSON highlight & HTTP diagnostics

### Added
- **`foreach` step** — iterate a step over every element of an extracted JSON array. Add `foreach = "{{VAR}}"` on any step; `{{item}}` is the current element and `{{item_index}}` its 0-based position:

  ```toml
  [[steps]]
  name    = "List users"
  url     = "https://api.example.com/users"
  [steps.extract]
  user_ids = "*.id"          # collects all id values → JSON array

  [[steps]]
  name    = "Get profile"
  foreach = "{{user_ids}}"
  url     = "https://api.example.com/users/{{item}}/profile"
  ```

  - Live progress: `✓ Get profile [3/10]` for each iteration
  - `continue_on_error` and assertions apply per iteration
  - Output connector collects all N bodies into the JSON array
  - Campaign idle view shows a `↻` badge on foreach steps

- **`*` wildcard in extraction paths** — `data.*.id` maps over an array and returns a new JSON array of all matching values. Combines naturally with `foreach`:
  - `"*.id"` → extracts all `id` fields from the root array
  - `"items.*.price"` → extracts all `price` from `items` array
  - Works recursively: `"a.*.b.*.c"` chains multiple wildcards

- **`include_vars` in output connector** — a campaign `[[outputs]]` block can now carry identifying context alongside each response body:

  ```toml
  [[outputs]]
  from_step    = "Get weather"
  path         = "results.json"
  include_vars = ["city", "country", "lat", "lon"]
  ```

  Each output object becomes `{ "body": {...}, "city": "Paris", "country": "FR", … }`.

- **JSON syntax highlighting** — Raw and HTTP response views now colour-code JSON content (no new dependencies — pure Rust char-by-char tokenizer):
  - Keys → Cyan bold
  - Strings → Green
  - Numbers → Yellow
  - `true` / `false` → Magenta
  - `null` → Dark grey
  - Braces / brackets → Indexed(240) bold

- **HTTP view diagnostics section** — a new `── Diagnostics ──` section at the bottom of the HTTP response view shows:
  - **Elapsed** — response time in ms, colour-coded: green < 300 ms, yellow < 1 s, red ≥ 1 s
  - **Size** — response body size (B / KB / MB) with `(decompressed)` if `Content-Encoding` was present
  - **Type** — `Content-Type` from response headers
  - **Encoding** — `Content-Encoding` if present
  - **Server** — `Server` header if present

- **Transport error display in HTTP view** — when a request fails at the transport layer (TLS failure, DNS error, connection refused, timeout), the HTTP view now shows:
  - `⚠  Transport error` in red bold
  - The full error chain (each `caused by:` line) formatted inline with indentation
  - Elapsed time (if available, e.g. for timeouts)

### Changed
- Campaign panel: switching campaign in the left list now resets the right panel to **Idle** — the previous run result is cleared. Previously, the Done panel from the last run was still visible when selecting a different campaign.
- Campaign idle view: GraphQL steps display a magenta `GQL` badge instead of `POST`, matching the rest of the TUI.

### Added (examples)
- **`examples/campaigns/eu_capitals.toml`** — full 4-step pipeline: GraphQL seed (53 EU countries from countries API) → language transform → geocode capital (IGN Géoplateforme) → live weather (Open-Meteo). Output includes `include_vars` with country metadata. Paired with `examples/campaigns/eu_capitals_map.html`.
- **`examples/campaigns/eu_capitals_map.html`** — dark-themed Leaflet.js interactive map. Reads `eu_capitals_weather.json` and renders each capital as a coloured bubble (temperature scale blue → red) with flag emoji, weather icon, and a full detail popup. Served locally via `python3 -m http.server 8080 --directory examples`.
- **`examples/campaigns/foreach_demo.toml`** — demonstrates `foreach`: GET /users → `*.id` wildcard extraction → foreach GET /todos per user.

---

## [0.7.0] — 2026-06-24 — OAuth2 (Client Credentials + Authorization Code)

### Added
- **OAuth2 Client Credentials** — nouvel `AuthType` dans l'onglet Auth. Configurer Token URL, Client ID, Client Secret et Scope (optionnel). Le token est obtenu automatiquement avant l'envoi de la requête (POST `application/x-www-form-urlencoded`, `grant_type=client_credentials`). Le token est mis en cache en session avec gestion de l'expiration (`expires_in`).

- **OAuth2 Authorization Code** — flow complet en TUI. Configurer Token URL, Client ID, Client Secret, Scope, Auth URL et Redirect Port (défaut : 9876). Terapi ouvre le navigateur avec l'URL d'autorisation, démarre un serveur TCP local temporaire pour capturer le `code`, l'échange contre un token, puis envoie la requête. Timeout 5 min.

- **Touches Auth tab** :
  - `↑`/`↓` navigue entre les champs ; `Space`/`Enter` cycle le type ou ouvre l'éditeur de champ
  - `f` — fetch manuel du token OAuth2 (sans envoyer la requête)
  - `Esc` — annule l'attente du callback navigateur ou efface une erreur OAuth2

- **Indicateur de statut token** — ligne `● token cached` (vert) ou `○ no token  (f to fetch)` (gris) affichée dans le panneau Auth. Banner jaune `⟳ fetching…` / `⟳ waiting for browser…` pendant l'obtention.

- **Persistance TOML** — tous les champs OAuth2 (`oauth2_token_url`, `oauth2_client_id`, `oauth2_client_secret`, `oauth2_scope`, `oauth2_auth_url`, `oauth2_redirect_port`) sont sauvegardés dans le TOML de la collection. Compat ascendante garantie via `#[serde(default)]`. Le token lui-même n'est jamais écrit sur disque (session uniquement).

### Changed
- L'hint de l'onglet Auth mentionne désormais `f: fetch token`

### Fixed
- **Race condition sur la clé de cache OAuth2** — si l'utilisateur modifiait les champs auth pendant qu'un fetch asynchrone était en cours, le token était stocké sous la mauvaise clé (la config courante au moment de l'insertion, pas celle au moment du fetch). La clé est maintenant calculée avant le `tokio::spawn` et transportée avec le résultat dans le canal.
- **CC et AC partageaient la même clé de cache** — deux flows OAuth2 avec les mêmes `token_url` et `client_id` mais des types différents (Client Credentials vs Authorization Code) écrasaient mutuellement leur cache. La clé inclut désormais le type d'auth (`auth_type:token_url:client_id`).
- **Type selector Auth** — `OAuth2 CC` et `OAuth2 AC` absents de la liste de sélection dans l'onglet Auth. Labels courts ajoutés dans le sélecteur.

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

- **Itinerary campaign example** (`examples/campaigns/itineraire_demo.toml`) — demonstrates the full params + pipeline flow: geocode two French cities via the IGN Géoplateforme API, compose coordinates with a transform step, then compute the road itinerary (distance + duration). No API key required. `DEPART`, `ARRIVEE`, `PROFILE`, and `OPTIMIZATION` are declared as `[[params]]` so each run can target different cities.

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
- **JSON connector** (`type = "json"`) — new campaign connector type that iterates over a JSON array. `path` points to a local JSON file; `select` (optional dot-path) navigates to the target array inside the file (omit or set to `""` for root). Object fields are flattened with dot-notation; nested arrays serialised as JSON strings. See `examples/campaigns/json_connector_demo.toml`.
- **Seed step** (`kind = "seed"`) — a campaign step that runs once before the iteration loop and whose JSON response body feeds the `[[connectors]]` block via `from_step = "step name"`. Enables fully HTTP-driven data-driven campaigns without a local file. The seed step is skipped in the iteration loop. See `examples/campaigns/seed_step_demo.toml`.
- **Output connector** (`[[outputs]]`) — after all iterations complete, writes a JSON array of step response bodies to disk. Fields: `from_step` (step name to collect), `path` (destination file), `select` (optional dot-path into each response body). Failed iterations are skipped. Parent directories created if needed. Multiple `[[outputs]]` blocks supported per campaign. CLI confirms each written file at the end of the report.
- **New campaign examples** — `examples/campaigns/json_connector_demo.toml` (JSON file connector, JSONPlaceholder), `examples/campaigns/seed_step_demo.toml` (seed step + output connector, French geo API), `examples/campaigns/users.json` (sample data).

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
  - `examples/campaigns/crud_demo.toml` — full CRUD on JSONPlaceholder (POST → extract id → GET → PUT → PATCH → DELETE)
  - `examples/campaigns/auth_flow.toml` — ReqRes auth flow (login → extract token → GET user → PUT update)
  - `examples/campaigns/debug_toolbox.toml` — httpbin.io edge cases (status codes, headers, bearer auth)
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
- `examples/collections/collection.toml` — annotated template documenting the collection TOML format
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
- `examples/campaigns/users.toml` — campaign with login → JWT extraction → CRUD steps
- `examples/campaigns/bulk_invite.toml` — data-driven campaign with CSV connector
- `examples/campaigns/contacts.csv` — sample contact list for bulk_invite
