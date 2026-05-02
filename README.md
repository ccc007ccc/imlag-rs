# imlag-rs

[![License: GPL v3](https://img.shields.io/badge/License-GPLv3-blue.svg)](LICENSE)
[![CI](https://github.com/ccc007ccc/imlag-rs/actions/workflows/ci.yml/badge.svg)](https://github.com/ccc007ccc/imlag-rs/actions)

**English** · [简体中文](README.zh-CN.md)

> Auto-excuse-after-death tool for Counter-Strike 2 — Tauri rewrite.

ImLag listens to CS2's [Game State Integration][gsi] feed and, the
moment you die, fires a randomly picked excuse (_"lag"_, _"my mouse"_,
_"got distracted"_, …) into the chat. Your post-mortem mental health
restored, automatically.

This is a Rust + Tauri rewrite of the original [Godot + C# project][orig].
The GSI parser is split into its own open-source crate:
[`cs2-gsi`](https://github.com/ccc007ccc/cs2-gsi).

![ImLag screenshot](image.png)

---

## Features

- **GSI auto-setup** — writes `gamestate_integration_ImLag.cfg` into the
  CS2 cfg directory on first launch, no manual config required.
- **Win11 acrylic UI** — Tauri 2 webview with `windowEffects: ["acrylic"]`,
  custom CS2-flavoured palette (tactical orange accent), Fluent-2 surfaces.
- **Three UI languages** — `zh-CN` / `zh-TW` / `en`, hot-swappable from
  the title bar globe button.
- **Corpus management** — import from a file or URL (plain text or JSON
  array), de-duplicated automatically.
- **Two trigger modes**
  - **CFG mode** — patches `autoexec.cfg` once with a single
    `bind "<trigger>" "exec imlag_say"`. The dispatch cfg is **empty between
    deaths**, so accidentally pressing the trigger key in-game is a no-op
    (no preset leaks). On every death ImLag rewrites the cfg with one
    `say "..."` / `say_team "..."` line, taps the trigger, waits ~300 ms,
    then clears the cfg again. Channel selection is `global` / `team` /
    `random`.
  - **Chat mode** — releases every held key first (so movement / lean /
    crouch don't bleed in), opens the chat box, pastes the message, hits
    Enter, via simulated keystrokes.
- **Safe cfg edits** — original `autoexec.cfg` is backed up before any
  modification; one-click restore removes every ImLag artefact.
- **Live status** — real-time GSI online state, watched-player death
  events, generated CFG group counts in the status bar.

---

## Quick start

### Prerequisites

| | Version |
|---|---|
| Rust | **1.75+** (stable) |
| Node.js | **18+** (Tauri builds the frontend) |
| Tauri CLI | `cargo install tauri-cli --version "^2.0" --locked` |
| Sibling repo | clone `cs2-gsi` next to this one — `Cargo.toml` uses a path dependency |

```powershell
# Layout: keep both repos in the same parent folder.
git clone https://github.com/ccc007ccc/cs2-gsi
git clone https://github.com/ccc007ccc/imlag-rs
```

### Run from source (dev)

```powershell
cd imlag-rs/crates/imlag-tauri
cargo tauri dev
```

Tauri's `beforeDevCommand` chains `npm --prefix frontend run dev` so the
Vite dev server (port 5173) is up before the webview attaches; React HMR
just works.

### Production build

```powershell
cd imlag-rs/crates/imlag-tauri
cargo tauri build
```

Outputs:

| | Path |
|---|---|
| Standalone exe | `target/release/imlag-tauri.exe` |
| MSI installer | `target/release/bundle/msi/ImLag_*_x64_*.msi` |
| NSIS installer | `target/release/bundle/nsis/ImLag_*_x64-setup.exe` |

> WebView2 Runtime is required at runtime. Win11 ships it; Win10 users may
> need to install it manually (Edge / Microsoft download).

### Without the Tauri CLI

```powershell
cd crates/imlag-tauri/frontend
npm run build
cd ..
cargo run --release
```

This bakes the latest `dist/` into the binary and starts it. You lose
hot-reload but don't have to install `tauri-cli`.

---

## Repository layout

```
imlag-rs/
├── Cargo.toml                 # workspace root
├── crates/
│   ├── imlag-core/            # business logic, no GUI deps
│   │   └── src/
│   │       ├── config.rs           # AppConfig — JSON, with PascalCase aliases
│   │       ├── chat.rs             # corpus (load / save / import / random)
│   │       ├── sender.rs           # chat-mode key simulation
│   │       ├── cfg_manager.rs      # CFG-mode .cfg generation + autoexec patch
│   │       ├── platform/           # Win32 keysim & foreground check (stub on others)
│   │       ├── i18n.rs             # backend translation (event/status messages)
│   │       ├── events.rs           # UiEvent broadcast for the webview
│   │       └── engine.rs           # ties cs2-gsi, corpus, sender, cfg_manager
│   └── imlag-tauri/           # Tauri 2 desktop shell
│       ├── src/                    # Rust side: commands, state, events bridge
│       ├── frontend/               # Vite + React 19 + Tailwind v4
│       │   └── src/
│       │       ├── App.tsx
│       │       ├── components/     # Button, Card, Toggle, Tabs, ListItem, …
│       │       ├── views/          # General / CFG / Chat / Corpus tabs
│       │       ├── layout/         # TitleBar, StatusBar
│       │       ├── lib/            # api, engine context, i18n, reveal effect
│       │       ├── styles/         # tokens.css (CS2 palette), globals.css
│       │       └── locales/        # zh-CN / zh-TW / en JSON dictionaries
│       ├── icons/                  # exe / installer icon set
│       └── tauri.conf.json
└── target/                    # cargo + tauri output
```

The GSI protocol layer lives in the sibling crate
[`cs2-gsi`](https://github.com/ccc007ccc/cs2-gsi) (path dependency
`../cs2-gsi`).

---

## Configuration

`Config.json` (auto-generated; legacy Godot `PascalCase` keys still load
thanks to serde aliases):

```json
{
  "playerNames": ["YourInGameName"],
  "onlySelfDeath": true,
  "triggerKey": "k",
  "cfgChatMode": "global",
  "cs2Path": "",
  "useCfgMode": true,
  "chatKey": "y",
  "keyDelay": 100,
  "skipWindowCheck": false,
  "forceMode": false,
  "language": "zh-CN",
  "autoStartGsi": true
}
```

| Field | Meaning |
|---|---|
| `playerNames` | Names whose deaths trigger a chat reply (compared against GSI `player.name`) |
| `onlySelfDeath` | Only fire when one of `playerNames` is the dying player |
| `triggerKey` | Single key bound to `exec imlag_say` in CFG mode |
| `cfgChatMode` | CFG dispatch channel: `"global"` / `"team"` / `"random"` |
| `useCfgMode` | `true` = CFG mode (cfg slot rewrite + trigger key); `false` = simulate keys directly |
| `chatKey` | Chat-mode opener (`y` for global, `u` for team) |
| `keyDelay` | Inter-key delay (ms), clamped to 30–1000 |
| `skipWindowCheck` | Bypass the "is CS2 the foreground window?" guard (not recommended) |
| `forceMode` | Press the chat-open key 3× to defeat occasional swallowed input |
| `language` | UI language: `zh-CN` / `zh-TW` / `en` |
| `autoStartGsi` | Start the GSI listener on launch |

> Older configs that still carry `bindKeys` / `teamBindKeys` / `preferTeamChat`
> keep loading: the first legacy key migrates into `triggerKey`, and
> `preferTeamChat: true` migrates into `cfgChatMode: "team"`.

`Messages.txt`: one corpus entry per line, UTF-8.

Storage location:
1. Current working directory if it already contains `Config.json` or
   `Messages.txt` (keeps the legacy Godot install layout working).
2. Otherwise `%APPDATA%\imlag\` (or platform equivalent via `directories`).

---

## Development

```powershell
# Workspace check
cargo check --workspace

# Tests (Rust side)
cargo test --workspace

# Lint
cargo clippy --workspace --all-targets -- -D warnings

# Format
cargo fmt --all

# Frontend type-check (fast smoke test, no bundler)
cd crates/imlag-tauri/frontend
npm run typecheck

# Frontend production build (just the dist/ folder)
npm run build
```

### Stack matrix

| Layer | Tech |
|---|---|
| GSI protocol | [`cs2-gsi`](https://github.com/ccc007ccc/cs2-gsi) (hyper 1.x + tokio) |
| Desktop shell | Tauri 2 + WebView2 |
| Frontend | React 19 + Vite 6 + TypeScript 5.6 + Tailwind v4 |
| Async runtime | tokio (multi-threaded) |
| Clipboard | Win32 OpenClipboard (in-tree, no `arboard`) |
| File dialog | `tauri-plugin-dialog` |
| Win32 input | `windows` crate 0.58 (KeyEvent injection, foreground check) |

---

## Acknowledgements

- Original project: [@cneicy/ImLag](https://github.com/cneicy/ImLag)
- Upstream GSI library inspiration: [antonpup/CounterStrike2GSI](https://github.com/antonpup/CounterStrike2GSI)
- The standalone GSI crate this app uses: [`cs2-gsi`](https://github.com/ccc007ccc/cs2-gsi)

## License

GPL-3.0-or-later — same as the original Godot project. See [LICENSE](LICENSE).

[gsi]: https://developer.valvesoftware.com/wiki/Counter-Strike_Global_Offensive_Game_State_Integration
[orig]: https://github.com/cneicy/ImLag
