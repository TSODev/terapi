# Terapi Builder — Design document

Interactive TUI editor for creating and editing Terapi campaigns. The builder is a first-class part of the `terapi` binary — no extra Cargo feature, no separate install. `terapi build` works out of the box.

---

## Invocation

```bash
terapi build                        # blank campaign, name editable in Campaign Settings
terapi build my_campaign.toml       # edit an existing file
```

If the file does not yet exist it is created on save (`w`).

---

## Layout

```
┌─ Builder: my_campaign.toml ──────────────────────────────────────────────────┐
│                                                                               │
│  ┌─ Pipeline · name [env] ─────┐  ┌─ [context panel] ──────────────────────┐ │
│  │  [CSV] connector.csv        │  │                                         │ │
│  │  # Section 1: auth          │  │   (Help / Catalog / Step editor /       │ │
│  │  [1] HTTP  GET   /health    │  │    Collection browser /                 │ │
│  │▶ [2] HTTP  POST  /login     │  │    Campaign settings /                  │ │
│  │       ⊘ if ROLE == "admin"  │  │    Variables / Checker /                │ │
│  │       ? status eq 200       │  │    TOML preview)                        │ │
│  │  [3] WAIT  500ms            │  │                                         │ │
│  │  [4] FILE  /img.png → DATA  │  │                                         │ │
│  │  ↷ continue-on-error        │  │                                         │ │
│  │  [OUT] output.json          │  │                                         │ │
│  └─────────────────────────────┘  └─────────────────────────────────────────┘ │
│                                                                               │
│  Builder › Pipeline [modified]  — Request "Login" loaded into step           │
│  n: new  i: insert  d: del  K/J: move  Enter: edit  s: settings  ...        │
└───────────────────────────────────────────────────────────────────────────────┘
```

- **Left (40%)** — pipeline, always visible, cursor `▶`
- **Right (60%)** — context panel, changes with active focus
- **Status bar 2 lines** — breadcrumb + active status message + keybinding hints

---

## Pipeline view

Each step is displayed on 1–3 lines depending on its content:

```
  [CSV] connector.csv              ← [IN] sections above steps
  [JSON] data.json

  # Section 1: authentication
  [1] HTTP  GET    https://api.example.com/health
  [2] TRSF  regex  → USER_ID
  [3] HTTP  POST   /users/{{item}}
       ↻ foreach: {{user_ids}}
       ⊘ if ROLE == "admin"
       ? status eq 201  ·  ? body.ok eq true
  [4] WAIT  1000ms
  [5] SEED  GET    /bootstrap
  [6] FILE  /img.png → DATA (base64)
  ↷ continue-on-error

  [OUT] output.json                ← [OUT] sections below steps
```

**Badges and colours:**

| Badge  | Colour  | Type                         |
|--------|---------|------------------------------|
| `HTTP` | cyan    | standard HTTP step           |
| `TRSF` | yellow  | transform                    |
| `WAIT` | grey    | pause                        |
| `SEED` | blue    | seed (feeds a connector)     |
| `FILE` | magenta | file loader (base64/text/hex)|
| `#`    | dark    | comment / separator          |

Secondary lines (indented, greyed):
- `↻ foreach: {{VAR}}` — iterating step
- `⊘ if VAR == "val"` — when condition
- `? assertion...` — assertions (up to 2, then `+N`)

**[IN] and [OUT] sections** appear above and below the numbered steps. They are navigable: pressing `↑` past step 0 enters `PipelineConnectors`; pressing `↓` past the last step enters `PipelineOutputs`. In those sub-states, `Enter` opens the editor, `d` deletes the selected item, and `Esc` returns to the pipeline.

Pipeline title: `Pipeline · campaign-name [active-env]`  
Pipeline footer: `↷ continue-on-error` flag when enabled at campaign level.

---

## Campaign Settings (`s`)

Opens from Pipeline with `s`. Edits the campaign-level fields.

```
┌─ Campaign Settings ──────────────────────────────────────┐
│                                                          │
│  ▶ Name              [ My Campaign           ]           │
│    Description       [ Fetches weather data  ]           │
│    Continue on error [ ] disabled                        │
│    Env               [ production ▾ ]                    │
│    Params            (0 params)                          │
│                                                          │
│  ↑↓: field  Enter: edit/toggle  ←/→: cycle env  Esc: back│
└──────────────────────────────────────────────────────────┘
```

| Field              | Behaviour                                             |
|--------------------|-------------------------------------------------------|
| Name               | Free text edit (Enter to open, Enter to confirm)      |
| Description        | Free text edit                                        |
| Continue on error  | Toggle with Space or Enter                            |
| Env                | Cycles through available terapi envs with ←/→ or Enter; `— none —` to clear |
| Params             | Navigates to ParamsEditor                             |

The selected env is stored as `env_file` in the campaign TOML. It is also shown in the Pipeline panel title.

---

## Catalog — bricks

```
┌─ Catalog — choose a brick ─────────────────────┐
│                                                 │
│  ▶ HTTP step        HTTP request                │
│    Transform        variable transform          │
│    Pause            wait (ms)                   │
│    Seed             seed connector              │
│    Comment          text note / separator       │
│    File Loader      read file → variable        │
│    Connector [IN]   CSV / JSON input source     │
│    Output [OUT]     collect responses to JSON   │
│                                                 │
│  ↑↓: choose  Enter: create  Esc: cancel         │
└─────────────────────────────────────────────────┘
```

- `foreach` is not a separate brick — it is a field in the HTTP/Seed step editor.
- **Connector** and **Output** do not create `[[steps]]` entries — they add `[[connectors]]` / `[[outputs]]` TOML blocks and appear in the [IN] / [OUT] pipeline sections.

---

## Comment steps

A **Comment** brick inserts a visible annotation between steps. It is not a real step:

- **In the pipeline** — displayed as `# Comment text here` in dark grey, no number, no badge; the cursor (`▶`) can select it for editing or deletion
- **In the step editor** — only one field: *Name* (the comment text)
- **In TOML output** — rendered as `# Comment text here` between `[[steps]]` blocks
- **In the campaign runner** — silently skipped (`kind == "comment"`)

Use comments to document sections of the pipeline without affecting execution.

---

## Loading a request from a collection

When creating an HTTP or Seed step, the `[L] Load from collection` row in the step editor opens the **Collection browser**. This is also the workflow for replacing an existing step's fields.

```
┌─ Collections — select a request ───────────────┐
│  ▼ Public APIs                                  │
│    ▼ Auth                                       │
│  ▶   POST   Login                               │
│      POST   Refresh                             │
│    ▶ Users                                      │
│  ▼ GraphQL APIs                                 │
│      POST   Introspection                       │
│                                                 │
│  ↑↓: navigate  Space: expand/collapse           │
│  Enter: load  Esc: cancel                       │
└─────────────────────────────────────────────────┘
```

`Enter` on a request → fills `Method`, `URL`, `Headers`, `Body` of the step under edit and returns to the step editor. `{{VAR}}` placeholders from the collection are preserved as-is.

All top-level collections are expanded by default when the browser opens.

**Checker note:** `{{VAR}}` imported from a collection come from the terapi environment, not necessarily from the campaign `[env]`. The checker will flag any variable used in the pipeline that is not defined in the campaign — this is one of its main contributions.

---

## Step editor — fields by type

### HTTP / Seed step

```
▶ Name              [ Get users                   ]
  Description       [ Fetch all users from API    ]
  Method            [ GET ▾ ]
  URL               [ https://api.example.com/{{BASE}}/users ]
  Headers           (2 items)  a: add  d: del
     Content-Type: application/json
     Authorization: Bearer {{TOKEN}}
  Body              —          (Enter: multi-line textarea)
  Extract           (1 item)  a: add  d: del
     user_ids = data.*.id
  Assertions        (2 items)  a: add  d: del
     status eq 200
     body.ok eq true
  Foreach           [ {{user_ids}} ]
  When              —
  Continue on error [ ] disabled

  [L] Load from collection    Enter / L
```

**Body editing**: pressing `Enter` on the Body row opens a full-panel multi-line textarea (yellow border, powered by `tui-textarea`). `Esc` saves and returns to the step editor.

### Transform step

```
  Name              [ Normalize name   ]
  Description       [ Lowercase the raw name ]
  Kind              [ upper ▾ ]
  Input             [ {{raw_name}}     ]
  Output var        [ CLEAN_NAME       ]
```

### Pause step

```
  Name              [ Rate limit pause ]
  Description       —
  Wait (ms)         [ 1000             ]
```

### Comment step

```
  Name              [ # Section 2: user operations ]
```

### File Loader step

```
  Name              [ Load logo image  ]
  Description       —
  File path         [ /path/to/logo.png ]
  Output var        [ FILE_DATA        ]
  Encoding          [ base64 ▾ ]       (cycles: base64 / text / hex)
```

The encoded file content is stored as a campaign variable (default `FILE_DATA`) and can be referenced as `{{FILE_DATA}}` in a subsequent HTTP step body or header. Encoding defaults to `base64` and is omitted from TOML when at default. The output variable name is omitted when `FILE_DATA`.

**Use case**: load a binary or image file and POST it in a multipart-ready body. Actual multipart form-data assembly (Content-Type boundary, part boundaries) is planned for a future step type; for now the base64 content can be embedded directly in JSON bodies or custom form fields.

---

## Connectors [IN]

Added from Catalog (`Connector [IN]`). Defines a `[[connectors]]` TOML block (CSV or JSON data source that drives the pipeline).

```
┌─ Connectors ─────────────────────────────────────────┐
│  ▶ 1  CSV   data/users.csv   select: all             │
│    2  JSON  data/products.json                       │
│                                                      │
│  a: add  d: delete  Enter: edit  Esc: Pipeline       │
└──────────────────────────────────────────────────────┘
```

Fields per connector: `kind` (CSV/JSON), `path`, `select` (column filter), `from_step` (seed step name).

In the pipeline the [IN] section is navigable (`↑` from step 0 → `PipelineConnectors`). `Enter` opens ConnectorsEditor, `d` deletes, `Esc` returns to Pipeline.

---

## Outputs [OUT]

Added from Catalog (`Output [OUT]`). Defines an `[[outputs]]` TOML block (collects step response bodies to a JSON file).

```
┌─ Outputs ────────────────────────────────────────────┐
│  ▶ 1  [OUT]  login_step  → results.json              │
│                                                      │
│  a: add  d: delete  Enter: edit  Esc: Pipeline       │
└──────────────────────────────────────────────────────┘
```

Fields per output: `from_step` (step name, chosen via picker), `path`, `select`, `include_vars`.

**from_step picker**: adding or editing an output opens a step picker that lists only HTTP/Seed steps (transforms, pauses, file loaders, and comments are excluded). The user selects from the list; no free-text entry.

In the pipeline the [OUT] section is navigable (`↓` from last step → `PipelineOutputs`). `Enter` re-opens the picker, `d` deletes, `Esc` returns to Pipeline.

---

## Variables panel (`v`)

Manages the `[env]` block of the campaign TOML. Full CRUD: add, edit (rename + change value), delete.

```
┌─ Variables [env] ──────────────────────────────┐
│  BASE_URL    https://api.example.com            │
│▶ TOKEN       {{SECRET}}                         │
│  TIMEOUT     30                                 │
│                                                 │
│  a: add  d: del  Enter: edit  Esc: close        │
└────────────────────────────────────────────────┘
```

`Enter` on a variable opens an inline edit form (key + value, `Tab` switches fields). Renaming the key is supported — the old key is removed and the new one created.

---

## Checker (`c`)

Static analysis of the pipeline. Displays a report in the right panel.

```
┌─ Check Report ─────────────────────────────────┐
│  ✓  All variables resolved                      │
│  ✗  [3] {{user_ids}} not defined upstream       │
│  ✓  When conditions consistent                  │
│  ⚠  [5] URL without extract or assertion        │
│  ✓  Assertions syntactically valid              │
│                                                 │
│  Esc: close                                     │
└────────────────────────────────────────────────┘
```

**Rules checked:**
- Every `{{VAR}}` in url/body/headers is defined in `[env]`, extracted by a previous step, or comes from a connector
- `foreach = "{{VAR}}"` references a previously extracted var
- `when.var` references an existing var
- Extraction dot-paths have valid syntax (`a.*.b`, not `a..b`)

---

## TOML preview (`p`)

Right panel shows the generated TOML in real time, scrollable. Comment steps appear as `# text` lines between `[[steps]]` blocks. `continue_on_error` and `env_file` are included when set. Connectors and outputs serialized as `[[connectors]]` / `[[outputs]]` blocks.

---

## Keybindings summary

### Pipeline

| Key      | Action                                    |
|----------|-------------------------------------------|
| `↑`/`↓`  | Navigate steps (wraps into [IN]/[OUT])   |
| `Enter`/`e` | Edit selected step                    |
| `n`      | New step (append) → Catalog               |
| `i`      | Insert after cursor → Catalog             |
| `d`      | Delete selected step                      |
| `K`      | Move step up                              |
| `J`      | Move step down                            |
| `s`      | Campaign settings                         |
| `v`      | Variables panel                           |
| `c`      | Run checker                               |
| `p`      | TOML preview                              |
| `w`      | Save                                      |
| `q`      | Quit                                      |

### Pipeline — [IN] / [OUT] navigation

Activated by pressing `↑` past step 0 (→ `PipelineConnectors`) or `↓` past the last step (→ `PipelineOutputs`).

| Key      | Action                                    |
|----------|-------------------------------------------|
| `↑`/`↓`  | Navigate connectors / outputs            |
| `Enter`  | Edit selected item                        |
| `d`      | Delete selected item                      |
| `Esc`    | Return to Pipeline                        |

### Catalog

| Key      | Action                       |
|----------|------------------------------|
| `↑`/`↓`  | Choose a brick              |
| `Enter`  | Create → Step editor         |
| `Esc`    | Cancel → Pipeline            |

### Campaign Settings

| Key        | Action                                 |
|------------|----------------------------------------|
| `↑`/`↓`   | Navigate fields                        |
| `Enter`    | Edit text / toggle boolean             |
| `Space`    | Toggle Continue on error               |
| `←`/`→`   | Cycle env (Env field)                  |
| `Esc`      | Back to Pipeline                       |

### Step editor

| Key      | Action                                        |
|----------|-----------------------------------------------|
| `↑`/`↓`  | Navigate fields                              |
| `Enter`  | Edit field / cycle selector                   |
| `←`/`→`  | Cycle values (Method, TransformKind, Encoding)|
| `a`/`d`  | Add / delete in list fields                   |
| `L`      | Open Collection browser (HTTP/Seed)           |
| `Esc`    | Back to Pipeline                              |

Body field: `Enter` opens a full multi-line textarea (yellow border). `Esc` saves.

### Collection browser

| Key      | Action                                  |
|----------|-----------------------------------------|
| `↑`/`↓`  | Navigate                               |
| `Space`  | Expand / collapse folder               |
| `Enter`  | Load request into step / expand folder |
| `Esc`    | Cancel                                 |

### Variables panel

| Key      | Action                    |
|----------|---------------------------|
| `↑`/`↓`  | Navigate variables       |
| `a`      | Add new variable          |
| `d`      | Delete selected           |
| `Enter`  | Edit (key + value form)   |
| `Tab`    | Switch Key ↔ Value field  |
| `Esc`    | Close / cancel edit       |

---

## Code architecture

```
src/
├── main.rs              # Commands::Build { file } => builder::run(file)
└── builder/
    ├── mod.rs           # BuilderApp struct, run(), event loop, all key handlers,
    │                    #   generate_toml(), new_step_for()
    ├── types.rs         # BuilderFocus, StepEditorMode, CampaignSettingsMode,
    │                    #   BrickKind, CheckResult, StepSection, PairTarget,
    │                    #   IoEditorMode, ParamEditorMode, VariablesMode
    ├── ui.rs            # rendering — pipeline ([IN]/steps/[OUT]), catalog,
    │                    #   step editor, body textarea, collection browser,
    │                    #   campaign settings, connectors editor, outputs editor,
    │                    #   output step picker, checker, toml preview,
    │                    #   variables panel, status bar
    ├── step_editor.rs   # sections_for(), handle_key(), current_value(),
    │                    #   apply_text_edit(), sorted_keys(), ensure_transform()
    ├── browser.rs       # BrowserNode, BrowserAddr, flatten(), handle_key(),
    │                    #   load_into_step()
    ├── checker.rs       # run() — static pipeline validation
    └── editor.rs        # move_step_up/down(), delete_step()
```

**Reused directly from Terapi:**
- `crate::storage::{load_collections, load_envs, StoredCollection}`
- `crate::campaign::{Campaign, Meta, Step, Transform, Assertion, StepCondition}`
- `crate::event::EventHandler` — keyboard + tick loop
- `tui-textarea` (reused `description_textarea` for body editor)

---

## Key types

```rust
pub struct BuilderApp {
    pub campaign: Campaign,                   // in-memory state
    pub path: Option<PathBuf>,                // target file
    pub cursor: usize,                        // selected step index
    pub focus: BuilderFocus,
    pub modified: bool,
    pub stored_collections: Vec<StoredCollection>,
    pub stored_env_names: Vec<String>,        // available envs for selector
    pub status_message: String,               // shown inline in status bar
    pub description_textarea: TextArea<'static>, // reused for body editing
    pub step_comments: Vec<String>,           // per-step TOML comments
    pub header_comment: String,               // campaign-level TOML header comment
}

pub enum BuilderFocus {
    Pipeline,
    Catalog          { insert_after: Option<usize>, cursor: usize },
    StepEditor       { step_idx: usize, section_cursor: usize, sub_cursor: usize,
                       mode: StepEditorMode, desc_active: bool },
    CollectionBrowser{ for_step: usize, col_cursor: usize, expanded: HashSet<String> },
    CampaignSettings { cursor: usize, mode: CampaignSettingsMode },
    Variables        { cursor: usize, mode: VariablesMode },
    Checker          { results: Vec<CheckResult> },
    TomlPreview      { scroll: usize },
    Run              { scroll: usize },
    ParamsEditor     { cursor: usize, mode: ParamEditorMode },
    ConnectorsEditor { cursor: usize, mode: IoEditorMode },
    OutputsEditor    { cursor: usize, mode: IoEditorMode },
    PipelineConnectors { cursor: usize },    // [IN] navigation sub-state
    PipelineOutputs  { cursor: usize },      // [OUT] navigation sub-state
    OutputStepPicker { output_idx: Option<usize>, step_cursor: usize,
                       f1: String, f2: String, f3: String, output_cursor: usize },
}

pub enum VariablesMode {
    Browse,
    Edit { original_key: Option<String>, key: String, value: String, field: u8 },
}

pub enum StepEditorMode {
    Browse,
    EditText   { buffer: String },
    EditBody,                                // full-panel textarea (tui-textarea)
    AddPairStage1 { target: PairTarget, buffer: String },
    AddPairStage2 { target: PairTarget, key: String, buffer: String },
    AddAssertPath { buffer: String },
    AddAssertOp   { path: String, op: usize },
    AddAssertValue{ path: String, op: usize, buffer: String },
    EditWhenVar   { buffer: String },
    EditWhenOp    { var: String, op: usize },
    EditWhenValue { var: String, op: usize, buffer: String },
}

pub enum BrickKind { Http, Transform, Pause, Seed, Comment, FileLoader, Connector, Output }

pub enum StepSection {
    Name, Description, Method, Url, Body, Headers, Extract, Assertions,
    Foreach, When, ContinueOnError, WaitMs,
    TransformKind, TransformInput, TransformOutput,
    FilePath, FileOutput, FileEncoding,
    LoadFromCollection,
}
// sections_for("http")      → Name, Description, Method, Url, Headers, Body,
//                              Extract, Assertions, Foreach, When, ContinueOnError, LoadFromCollection
// sections_for("pause")     → Name, Description, WaitMs
// sections_for("transform") → Name, Description, TransformKind, TransformInput, TransformOutput
// sections_for("file")      → Name, Description, FilePath, FileOutput, FileEncoding
// sections_for("comment")   → Name only

pub enum PairTarget { Headers, Extract }

pub struct CheckResult {
    pub level: CheckLevel,         // Ok | Warning | Error
    pub step_idx: Option<usize>,
    pub message: String,
}
```

---

## Roadmap Builder

### Implemented (v0.8 builder)

- [x] `BuilderApp` struct + event loop + base layout (40/60 split + 2-line status bar)
- [x] Pipeline view — numbered steps, badges, secondary lines (foreach/when/assert)
- [x] Comment steps — `# text` in pipeline, rendered as TOML comment, skipped by runner
- [x] Catalog — brick selection, blank step creation
- [x] Collection browser — full tree navigation, expand/collapse, load into step
- [x] Step editor — HTTP step (all fields including assertions + when + foreach, two-stage pair add for headers/extract)
- [x] Step editor — Transform / Pause / Seed / Comment
- [x] Step editor — **File Loader** (`kind = "file"`) — file path, output variable, encoding cycle (base64/text/hex); execution in `campaign.rs` reads file, encodes, stores in env variable
- [x] **Body editor** — multi-line `tui-textarea` (yellow border, `Esc` to save); fixes body silently dropped in earlier `generate_toml`
- [x] **Transforms serialization** — inline TOML table array with all sub-fields (kind/input/output/pattern/group/from/to/delimiter/index)
- [x] `[L] Load from collection` — opens browser, loads method/URL/headers/body
- [x] Move (K/J), Delete, Insert after cursor
- [x] Campaign settings (`s`) — name, description, continue_on_error, env_file, params
- [x] **Variables panel** (`v`) — full CRUD: browse, add, edit (rename + value, Tab switches fields), delete; `VariablesMode` enum
- [x] **Connectors [IN]** — added from Catalog; `[[connectors]]` TOML blocks; `ConnectorsEditor` + `PipelineConnectors` navigation; `↑` from step 0 enters [IN] section
- [x] **Outputs [OUT]** — added from Catalog; `[[outputs]]` TOML blocks; `OutputsEditor` + `PipelineOutputs` navigation; `↓` from last step enters [OUT] section; `OutputStepPicker` for `from_step` (filters to HTTP/Seed only)
- [x] TOML preview (`p`) — generated TOML with comments, connectors, outputs
- [x] Checker (`c`) — static variable resolution check
- [x] Save (`w`) — writes TOML to path or `<terapi_dir>/campaigns/`

### Planned — next iterations

- [ ] **Quit confirmation** — prompt `Save before quit? y/n/Esc` when `modified = true`
- [ ] **Run step** (`r` in step editor) — execute the current HTTP step's request from the builder; display response in right panel; reuse `execute_http()` via a tokio spawn + mpsc channel
- [ ] **JSON path autocomplete** — after running a step, extract all dot-paths from the response; offer autocompletion when editing Extract value fields in subsequent steps
- [ ] **Checker improvements** — validate connector `from_step` references, `[[outputs]]` `from_step` existence, `file_path` non-empty for File Loader steps
- [ ] **TOML syntax highlight** in preview (reuse `highlight_raw()` from `ui.rs`)
- [ ] **Multipart form-data step** — dedicated step type to assemble `multipart/form-data` bodies from variables (including base64 content from File Loader); sets Content-Type with boundary automatically
