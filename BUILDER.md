# Terapi Builder — Design document

Éditeur TUI interactif pour créer et modifier des campaigns Terapi. Le builder fait partie intégrante du binaire `terapi` — aucune feature Cargo supplémentaire, aucune installation séparée. `terapi build` fonctionne out of the box.

---

## Invocation

```bash
terapi build                        # nouveau campaign, nom demandé au démarrage
terapi build mon_campaign.toml      # édition d'un fichier existant
```

Si le fichier n'existe pas encore, il est créé à la sauvegarde (`w`).

---

## Layout général

```
┌─ Builder: mon_campaign.toml ─────────────────────────────────────────────────┐
│                                                                               │
│  ┌─ Pipeline ──────────────────┐  ┌─ [panneau contextuel] ─────────────────┐ │
│  │  [1] HTTP  GET  /users      │  │                                         │ │
│  │  [2] TRSF  upper → NAME     │  │   (Catalog / Step editor /              │ │
│  │▶ [3] HTTP  POST /notify     │  │    Collection browser /                 │ │
│  │       ⊘ if ROLE == "admin"  │  │    Variables / Checker /                │ │
│  │       ? status == 201       │  │    TOML preview)                        │ │
│  │  [4] WAIT  500ms            │  │                                         │ │
│  │                             │  │                                         │ │
│  │  ● 2 vars · ✗ 1 warning     │  │                                         │ │
│  └─────────────────────────────┘  └─────────────────────────────────────────┘ │
│                                                                               │
│  n: new  i: insert  d: delete  K/J: move  Enter: edit  c: check  v: vars  w: save │
└───────────────────────────────────────────────────────────────────────────────┘
```

- **Gauche (40%)** : pipeline, toujours visible, curseur `▶`
- **Droite (60%)** : panneau contextuel, change selon le mode
- **Status bar 2 lignes** : breadcrumb + hints (même pattern que Terapi)

---

## Vue Pipeline

Chaque step affiché sur 1 à 3 lignes selon son contenu :

```
  [1] HTTP  GET    https://api.example.com/users
  [2] TRSF  regex  → USER_ID
  [3] HTTP  POST   /users/{{item}}
       ↻ foreach: {{user_ids}}
       ⊘ if ROLE == "admin"
       ? status eq 201  ·  ? body.ok eq true
  [4] WAIT  1000ms
  [5] SEED  POST   /bootstrap
```

**Badges couleurs :**

| Badge  | Couleur | Type                          |
|--------|---------|-------------------------------|
| `HTTP` | cyan    | step HTTP standard            |
| `TRSF` | yellow  | transform                     |
| `WAIT` | grey    | pause                         |
| `SEED` | blue    | seed (alimente un connector)  |

Lignes secondaires (indentées, grisées) :
- `↻ foreach: {{VAR}}` — step itératif
- `⊘ if VAR == "val"` — condition when
- `? assertion...` — assertions (jusqu'à 2, puis `+N`)
- `→ VAR` — extraction

Pied du panel :
```
● 3 vars  · ✓ pipeline OK     (ou ✗ 2 warnings)
```

---

## Les briques (Catalog)

```
┌─ Catalog ──────────────────────────────┐
│                                        │
│  ▶ HTTP step          requête HTTP     │
│    Transform          manipulation var │
│    Pause              attente (ms)     │
│    Seed               amorce connector │
│                                        │
│  ↑↓: choisir  Enter: créer  Esc: annul │
└────────────────────────────────────────┘
```

`foreach` n'est pas une brique séparée — c'est une option du HTTP step (champ dans l'éditeur).

---

## Création d'un HTTP step — point de départ

À la sélection de la brique HTTP step dans le Catalog, un choix intermédiaire est proposé :

```
┌─ Nouveau HTTP step ─────────────────────┐
│                                         │
│  ▶ Partir de zéro                       │
│    Charger depuis une collection        │
│                                         │
│  ↑↓: choisir  Enter: continuer          │
└─────────────────────────────────────────┘
```

- **Partir de zéro** → step editor vide, tous les champs à remplir manuellement
- **Charger depuis une collection** → collection browser s'ouvre ; `Enter` sur une requête pré-remplit le step editor avec méthode, URL, headers, body, auth — les `{{VAR}}` sont conservées telles quelles

Ce second chemin est le workflow dominant : une requête déjà testée dans Terapi est intégrée dans le campaign avec extraction et assertions. Le builder joue alors le rôle d'assembleur de requêtes existantes.

**Note checker :** les `{{VAR}}` issues d'une collection viennent de l'environnement Terapi, pas nécessairement du `[env]` du campaign. Le checker signalera toute variable utilisée dans le pipeline mais non définie dans le campaign — c'est l'un de ses apports principaux.

Ce choix initial (zéro / collection) est aussi accessible à tout moment via `L` depuis le step editor d'un HTTP ou Seed step déjà créé.

---

## Step editor — champs par type

### HTTP step

```
  Name          [Get users                  ]
  Method        [ GET ▾ ]
  URL           [https://api.example.com/{{BASE}}/users]
  ──────────────────────────────────────────────────────
  Headers       a: add  d: del
    Content-Type: application/json
  ──────────────────────────────────────────────────────
  Body          [ Text mode ]  (t: toggle JSON/Text)
  ──────────────────────────────────────────────────────
  Extract       a: add  d: del
    user_ids = data.*.id
  ──────────────────────────────────────────────────────
  Assertions    a: add  d: del
    status eq 200
    body.ok eq true
  ──────────────────────────────────────────────────────
  Foreach       [ {{user_ids}}           ]  (ou vide)
  When          [ var=ROLE eq="admin"    ]  (ou vide)
  Continue      [ ] continue on error
  ──────────────────────────────────────────────────────
  [L] Charger / remplacer depuis une collection
```

### Transform step

```
  Name          [Normalize name        ]
  Kind          [ upper ▾ ]  (template/regex/replace/split/trim/upper/lower)
  Input         [ {{raw_name}}         ]
  Output var    [ CLEAN_NAME           ]
  ── champs spécifiques au kind ─────────
  (pattern, replacement, index… selon kind)
```

### Pause step

```
  Name          [Rate limit pause      ]
  Wait (ms)     [ 1000                 ]
```

### Seed step

```
  Name          [Bootstrap data        ]
  Method        [ GET ▾ ]
  URL           [https://api.example.com/seed]
  (idem HTTP pour headers/body/auth)
  ── Connector ─────────────────────────
  From step     [Bootstrap data        ]
  Output path   [results.json          ]
  Select        [data.items            ]
```

---

## Collection browser

S'ouvre via `L` depuis le Step editor d'un HTTP/Seed step.

```
┌─ Collections ──────────────────────────────┐
│  ▼ Public APIs                             │
│    ▼ Auth                                  │
│  ▶   Login  POST  /auth/login              │
│      Refresh  POST  /auth/refresh          │
│    ▶ Users                                 │
│  ▶ GraphQL APIs                            │
│                                            │
│  Enter: charger  Esc: annuler              │
└────────────────────────────────────────────┘
```

`Enter` sur une requête → remplit `Method`, `URL`, `Headers`, `Body` du step en cours d'édition.

---

## Variables panel (`v`)

Gère le bloc `[env]` du campaign TOML. Même UX que l'onglet Env de Terapi.

```
┌─ Variables [env] ──────────────────────────┐
│  BASE_URL    https://api.example.com       │
│▶ TOKEN       {{SECRET}}                    │
│  TIMEOUT     30                            │
│                                            │
│  a: add  d: del  Enter: edit  Esc: fermer  │
└────────────────────────────────────────────┘
```

---

## Checker (`c`)

Analyse statique du pipeline, affiche un rapport dans le panneau droit.

```
┌─ Check report ─────────────────────────────┐
│  ✓  Variables résolues                     │
│  ✗  [3] {{user_ids}} non définie en amont  │
│  ✓  Conditions when cohérentes             │
│  ⚠  [5] URL sans extraction ni assert      │
│  ✓  Assertions syntaxiquement valides      │
│  ✓  Foreach référence une var existante    │
│                                            │
│  2 erreurs · 1 avertissement               │
│  Esc: fermer                               │
└────────────────────────────────────────────┘
```

**Règles vérifiées :**
- Toute `{{VAR}}` dans url/body/headers est définie dans `[env]`, extraite par un step précédent, ou issue d'un connector
- Un `foreach = "{{VAR}}"` référence une var extraite antérieurement
- Un `when.var` référence une var existante
- Les dot-paths d'extraction ont une syntaxe valide (`a.*.b`, pas `a..b`)
- Un step SEED a bien un `from_step` cohérent dans un `[[outputs]]`

---

## TOML preview (`p`)

Panneau droit affiche le TOML généré en temps réel, scrollable, avec syntaxe highlighting (réutilise `highlight_raw()` de Terapi).

---

## Clavier — récapitulatif

### Focus Pipeline

| Touche  | Action                               |
|---------|--------------------------------------|
| `↑`/`↓` | Naviguer les steps                  |
| `Enter`/`e` | Éditer le step sélectionné      |
| `n`     | Nouveau step (fin de liste) → Catalog |
| `i`     | Insérer après le curseur → Catalog  |
| `d`     | Supprimer (confirmation)            |
| `K`     | Monter le step d'une position       |
| `J`     | Descendre le step d'une position    |
| `c`     | Lancer le checker                   |
| `v`     | Panel variables                     |
| `p`     | Preview TOML                        |
| `w`     | Sauvegarder le fichier              |
| `q`     | Quitter (confirmation si modifié)   |

### Focus Catalog

| Touche  | Action                      |
|---------|-----------------------------|
| `↑`/`↓` | Choisir une brique         |
| `Enter` | Créer → Step editor         |
| `Esc`   | Annuler → Pipeline          |

### Focus Step editor

| Touche  | Action                                   |
|---------|------------------------------------------|
| `↑`/`↓` | Naviguer les champs                     |
| `Enter` | Éditer le champ sélectionné              |
| `Tab`   | Champ suivant                            |
| `a`/`d` | Ajouter/supprimer dans les listes       |
| `L`     | Ouvrir Collection browser (HTTP/Seed)   |
| `Esc`   | Valider et retour Pipeline              |

### Focus Collection browser

| Touche  | Action                        |
|---------|-------------------------------|
| `↑`/`↓` | Naviguer                     |
| `Enter` | Expand dossier / charger req  |
| `Esc`   | Annuler                       |

---

## Architecture code

Le builder est un module de première classe dans le binaire `terapi`, au même titre que `app/` ou `campaign.rs`. Aucune feature Cargo, aucun binaire séparé.

```
src/
├── main.rs              # ajoute : Commands::Build { path } => builder::run(path)
└── builder/
    ├── mod.rs        # BuilderApp struct, run(), event loop
    ├── types.rs      # BuilderFocus, CheckResult, BrickKind…
    ├── ui.rs         # rendering (pipeline, catalog, editor, checker…)
    ├── checker.rs    # validation statique du pipeline
    └── editor.rs     # logique d'édition des steps (add, move, delete…)
```

**Réutilisation directe depuis Terapi :**
- `crate::storage::{load_collections, StoredCollection, Campaign}`
- `crate::app::types::{flatten_stored, METHODS}`
- `crate::json_highlight::highlight_raw()` pour la preview TOML
- `crate::event::EventHandler` pour le loop clavier/tick

---

## État interne (types clés)

```rust
struct BuilderApp {
    campaign: Campaign,        // état en mémoire
    path: Option<PathBuf>,     // fichier cible
    cursor: usize,             // step sélectionné
    focus: BuilderFocus,
    modified: bool,            // changements non sauvegardés
    stored_collections: Vec<StoredCollection>,  // pour le browser
}

enum BuilderFocus {
    Pipeline,
    Catalog { insert_after: Option<usize> },
    StepEditor { step_idx: usize, field_cursor: usize, editing: bool },
    CollectionBrowser { for_step: usize, col_cursor: usize, expanded: HashSet<String> },
    Variables { cursor: usize },
    Checker { results: Vec<CheckResult> },
    TomlPreview { scroll: usize },
}

enum BrickKind {
    Http,
    Transform,
    Pause,
    Seed,
}

struct CheckResult {
    level: CheckLevel,   // Error | Warning | Ok
    step_idx: Option<usize>,
    message: String,
}

enum CheckLevel { Error, Warning, Ok }
```

---

## Roadmap Builder

- [ ] Squelette `BuilderApp` + boucle événements + layout de base
- [ ] Vue Pipeline — affichage numéroté, badges, lignes secondaires
- [ ] Catalog — sélection de brique, création de step vide
- [ ] Choix initial HTTP step : zéro vs collection
- [ ] Collection browser — navigation + chargement dans step editor
- [ ] Step editor — HTTP step (champs principaux + `L` pour recharger)
- [ ] Step editor — Transform / Pause / Seed
- [ ] Move (K/J), Delete, Insert
- [ ] Variables panel (v)
- [ ] TOML preview (p) — génération + highlight
- [ ] Checker (c) — validation statique + détection vars non définies
- [ ] Sauvegarde (w) + confirmation quitter (q)
