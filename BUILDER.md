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
│  │  # Section 1: auth          │  │                                         │ │
│  │  [1] HTTP  GET   /health    │  │   (Help / Catalog / Step editor /       │ │
│  │▶ [2] HTTP  POST  /login     │  │    Collection browser /                 │ │
│  │       ⊘ if ROLE == "admin"  │  │    Campaign settings /                  │ │
│  │       ? status eq 200       │  │    Variables / Checker /                │ │
│  │  [3] WAIT  500ms            │  │    TOML preview)                        │ │
│  │  ↷ continue-on-error        │  │                                         │ │
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
  # Section 1: authentication
  [1] HTTP  GET    https://api.example.com/health
  [2] TRSF  regex  → USER_ID
  [3] HTTP  POST   /users/{{item}}
       ↻ foreach: {{user_ids}}
       ⊘ if ROLE == "admin"
       ? status eq 201  ·  ? body.ok eq true
  [4] WAIT  1000ms
  [5] SEED  GET    /bootstrap
  ↷ continue-on-error
```

**Badges and colours:**

| Badge  | Colour  | Type                         |
|--------|---------|------------------------------|
| `HTTP` | cyan    | standard HTTP step           |
| `TRSF` | yellow  | transform                    |
| `WAIT` | grey    | pause                        |
| `SEED` | blue    | seed (feeds a connector)     |
| `#`    | dark    | comment / separator          |

Secondary lines (indented, greyed):
- `↻ foreach: {{VAR}}` — iterating step
- `⊘ if VAR == "val"` — when condition
- `? assertion...` — assertions (up to 2, then `+N`)

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
│                                                 │
│  ↑↓: choose  Enter: create  Esc: cancel         │
└─────────────────────────────────────────────────┘
```

`foreach` is not a separate brick — it is a field in the HTTP/Seed step editor.

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
  Method            [ GET ▾ ]
  URL               [ https://api.example.com/{{BASE}}/users ]
  Headers           (2 items)  a: add  d: del
     Content-Type: application/json
     Authorization: Bearer {{TOKEN}}
  Body              —
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

### Transform step

```
  Name              [ Normalize name   ]
  Kind              [ upper ▾ ]
  Input             [ {{raw_name}}     ]
  Output var        [ CLEAN_NAME       ]
```

### Pause step

```
  Name              [ Rate limit pause ]
  Wait (ms)         [ 1000             ]
```

### Comment step

```
  Name              [ # Section 2: user operations ]
```

---

## Variables panel (`v`)

Manages the `[env]` block of the campaign TOML.

```
┌─ Variables [env] ──────────────────────────────┐
│  BASE_URL    https://api.example.com            │
│▶ TOKEN       {{SECRET}}                         │
│  TIMEOUT     30                                 │
│                                                 │
│  a: add  d: del  Enter: edit  Esc: close        │
└────────────────────────────────────────────────┘
```

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

Right panel shows the generated TOML in real time, scrollable. Comment steps appear as `# text` lines between `[[steps]]` blocks. `continue_on_error` and `env_file` are included when set.

---

## Keybindings summary

### Pipeline

| Key     | Action                                    |
|---------|-------------------------------------------|
| `↑`/`↓` | Navigate steps                           |
| `Enter`/`e` | Edit selected step                   |
| `n`     | New step (append) → Catalog               |
| `i`     | Insert after cursor → Catalog             |
| `d`     | Delete selected step                      |
| `K`     | Move step up                              |
| `J`     | Move step down                            |
| `s`     | Campaign settings                         |
| `v`     | Variables panel                           |
| `c`     | Run checker                               |
| `p`     | TOML preview                              |
| `w`     | Save                                      |
| `q`     | Quit                                      |

### Catalog

| Key     | Action                       |
|---------|------------------------------|
| `↑`/`↓` | Choose a brick              |
| `Enter` | Create → Step editor         |
| `Esc`   | Cancel → Pipeline            |

### Campaign Settings

| Key        | Action                                 |
|------------|----------------------------------------|
| `↑`/`↓`   | Navigate fields                        |
| `Enter`    | Edit text / toggle boolean             |
| `Space`    | Toggle Continue on error               |
| `←`/`→`   | Cycle env (Env field)                  |
| `Esc`      | Back to Pipeline                       |

### Step editor

| Key     | Action                                     |
|---------|--------------------------------------------|
| `↑`/`↓` | Navigate fields                           |
| `Enter` | Edit field / cycle selector               |
| `←`/`→` | Cycle values (Method, TransformKind)      |
| `a`/`d` | Add / delete in list fields               |
| `L`     | Open Collection browser (HTTP/Seed)       |
| `Esc`   | Back to Pipeline                          |

### Collection browser

| Key     | Action                                  |
|---------|-----------------------------------------|
| `↑`/`↓` | Navigate                               |
| `Space` | Expand / collapse folder               |
| `Enter` | Load request into step / expand folder |
| `Esc`   | Cancel                                 |

---

## Code architecture

```
src/
├── main.rs              # Commands::Build { file } => builder::run(file)
└── builder/
    ├── mod.rs           # BuilderApp struct, run(), event loop, all key handlers
    ├── types.rs         # BuilderFocus, StepEditorMode, CampaignSettingsMode,
    │                    #   BrickKind, CheckResult, StepSection, PairTarget
    ├── ui.rs            # rendering — pipeline, catalog, step editor,
    │                    #   collection browser, campaign settings, checker,
    │                    #   toml preview, variables, status bar
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
}

pub enum BuilderFocus {
    Pipeline,
    Catalog { insert_after: Option<usize>, cursor: usize },
    StepEditor { step_idx: usize, section_cursor: usize, sub_cursor: usize, mode: StepEditorMode },
    CollectionBrowser { for_step: usize, col_cursor: usize, expanded: HashSet<String> },
    CampaignSettings { cursor: usize, mode: CampaignSettingsMode },
    Variables { cursor: usize },
    Checker { results: Vec<CheckResult> },
    TomlPreview { scroll: usize },
}

pub enum CampaignSettingsMode { Browse, EditText { buffer: String } }

pub enum StepEditorMode {
    Browse,
    EditText { buffer: String },
    AddPairStage1 { target: PairTarget, buffer: String },
    AddPairStage2 { target: PairTarget, key: String, buffer: String },
}

pub enum BrickKind { Http, Transform, Pause, Seed, Comment }

pub enum StepSection {
    Name, Method, Url, Body, Headers, Extract, Assertions,
    Foreach, When, ContinueOnError, WaitMs,
    TransformKind, TransformInput, TransformOutput,
    LoadFromCollection,
}
// sections_for("http")    → all 11 fields
// sections_for("pause")   → Name, WaitMs
// sections_for("transform") → Name, TransformKind, TransformInput, TransformOutput
// sections_for("comment") → Name only

pub enum PairTarget { Headers, Extract }

pub struct CheckResult {
    pub level: CheckLevel,         // Ok | Warning | Error
    pub step_idx: Option<usize>,
    pub message: String,
}
```

---

## Roadmap Builder

### Implemented

- [x] `BuilderApp` struct + event loop + base layout (40/60 split + 2-line status bar)
- [x] Pipeline view — numbered steps, badges, secondary lines (foreach/when/assert)
- [x] Comment steps — `# text` in pipeline, rendered as TOML comment, skipped by runner
- [x] Catalog — brick selection, blank step creation
- [x] Collection browser — full tree navigation, expand/collapse, load into step
- [x] Step editor — HTTP step (all 11 fields, two-stage add for headers/extract)
- [x] Step editor — Transform / Pause / Seed / Comment
- [x] `[L] Load from collection` — opens browser, loads method/URL/headers/body
- [x] Move (K/J), Delete, Insert after cursor
- [x] Campaign settings (`s`) — name, description, continue_on_error, env_file
- [x] Variables panel (`v`) — display campaign `[env]`
- [x] TOML preview (`p`) — generated TOML with comments and campaign-level fields
- [x] Checker (`c`) — static variable resolution check
- [x] Save (`w`) — writes TOML to path or `<terapi_dir>/campaigns/`

### Planned — v0.8 Builder Live Preview

- [ ] **Run step** (`r` in step editor) — execute the current step's HTTP request from the builder; display response in right panel
- [ ] **JSON path autocomplete** — after running a step, extract all JSON paths from the response; offer autocompletion when editing Extract value fields in subsequent steps
- [ ] **Variables panel** — add / delete / edit variables (currently display only)
- [ ] **Catalog → auto-open step editor** after creating a step (currently returns to Pipeline)
- [ ] **Quit confirmation** — prompt if modified and unsaved
- [ ] **TOML syntax highlight** in preview (reuse `highlight_raw()`)
