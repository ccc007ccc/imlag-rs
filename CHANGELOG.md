# Changelog

All notable changes to this project will be documented in this file.

## [Unreleased]

### Changed
- **CFG mode reworked** — replaced the preset-pool layout (`imlag_say_global_*.cfg`
  + `imlag_say_team_*.cfg` + selector cfgs) with a single runtime-rewritten
  dispatch slot `imlag_say.cfg`. The slot is empty between deaths so a stray
  press of the trigger key no longer leaks a preset message. ImLag rewrites
  the slot, presses the trigger, then clears it again on each death event.
- CFG mode now uses a single configurable `triggerKey` instead of separate
  `bindKeys` / `teamBindKeys` pools. Old configs migrate automatically: the
  first key from the legacy pools becomes the new trigger key.
- `preferTeamChat` is replaced by a tri-state `cfgChatMode`
  (`global` / `team` / `random`). `preferTeamChat: true` migrates to
  `cfgChatMode: "team"` on first load.

### Added
- `release_all_keys` Win32 helper — uses `GetKeyboardState` to release every
  key the OS currently considers pressed. Chat mode now calls it before
  typing, so movement / lean / crouch keys don't bleed into the chat box.
- `CfgChatMode::Random` — coin-flip per death between global and team chat.
- Unit tests covering the new dispatch slot lifecycle, legacy config
  migration, and trigger-key validation.

### Fixed
- CI: `actions/checkout` was rejecting the sibling `cs2-gsi` checkout via
  `path: ../cs2-gsi`; both jobs now check out into adjacent subdirectories
  with `working-directory: imlag-rs`.

### Initial port (pre-Unreleased baseline)
- Rust port of ImLag based on Godot 4 + C# original
- Modern dark UI built with Tauri 2 + React 19 + Tailwind v4
- Windows 11 Acrylic background via Tauri `windowEffects: ["acrylic"]`
- Cross-crate split:
  * `imlag-core` — config, corpus, CFG generator, key/clipboard automation
  * `imlag-tauri` — desktop application
- GSI integration via [`cs2-gsi`](https://github.com/ccc007ccc/cs2-gsi)
  (auto-writes `gamestate_integration_*.cfg` on first launch)

[Unreleased]: https://github.com/ccc007ccc/imlag-rs/commits/main
