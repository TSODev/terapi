# terapi

**Terminal + API** — a keyboard-driven TUI for exploring, testing, and automating REST and GraphQL APIs, without leaving your terminal.

```
┌─────────────────── terapi ───────────────────┐
│  Request  |  Collections  |  History          │
├──────────────────────────────────────────────┤
│ ┌─ URL ──────────────────────────────────── ┐│
│ │ GET  https://                              ││
│ └────────────────────────────────────────── ┘│
│ ┌─ Response ─────────────────────────────── ┐│
│ │                                            ││
│ │  Response will appear here…               ││
│ │                                            ││
│ └────────────────────────────────────────── ┘│
├──────────────────────────────────────────────┤
│ Ready — press q to quit, Tab to switch panels │
└──────────────────────────────────────────────┘
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
- **Pipeline automation** — chain requests, extract variables from responses, run assertions
- **Local-first** — collections stored as TOML, git-friendly, no account, no cloud
- **Single binary** — `cargo install terapi`, instant startup, zero Electron

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

## Keybindings

| Key | Action |
|-----|--------|
| `Tab` | Switch panel |
| `q` / `Esc` | Quit |

More keybindings will be added as features are implemented.

## Status

> **v0.1.0 — early skeleton.** The TUI boots and renders 3 panels. Everything else is coming.

This is a very early release to establish the crate name and project structure. See the roadmap below.

## Roadmap

### v0.2 — REST basics
- [ ] Method selector (GET / POST / PUT / PATCH / DELETE)
- [ ] URL input (editable)
- [ ] Headers editor
- [ ] Body editor (raw JSON)
- [ ] Send request (`Enter`) — async via tokio
- [ ] Response viewer: status, headers, pretty-printed JSON

### v0.3 — Collections
- [ ] TOML-based collection format
- [ ] Save / load requests
- [ ] Collections panel (list + select)

### v0.4 — Environment & History
- [ ] Environment variables (dev / staging / prod)
- [ ] Request history (persistent, TOML)

### v0.5 — GraphQL
- [ ] GraphQL mode toggle
- [ ] Schema introspection via `__schema`
- [ ] Query editor with field autocompletion
- [ ] Variables panel (JSON)
- [ ] Mutations support

### v0.6 — Automation / Scripting
- [ ] Chain requests (output of req N → input of req N+1)
- [ ] Variable extraction from JSON responses (JSONPath-style)
- [ ] Assertions (status code, body field values)
- [ ] Headless pipeline: `terapi run collection.toml`

### v1.0
- [ ] Auth: Bearer token, API Key, OAuth2 (basic)
- [ ] Syntax highlighting (syntect)
- [ ] Import from Postman collection (JSON v2.1)

## Stack

Built with Rust 2021:

| Role | Crate |
|------|-------|
| TUI rendering | `ratatui` + `crossterm` |
| HTTP client | `reqwest` (async) |
| Async runtime | `tokio` |
| Serialization | `serde` + `serde_json` |
| Error handling | `anyhow` |

## Contributing

Contributions, issues, and feature requests are welcome. This project is in early development — the best way to contribute right now is to open an issue describing what you'd like to see.

## License

MIT — © [TSODev](https://github.com/tsodev)
