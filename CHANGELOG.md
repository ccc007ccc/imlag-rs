# Changelog

All notable changes to this project will be documented in this file.

## [Unreleased]

### Added
- Initial Rust port of ImLag based on Godot 4 + C# original
- Modern dark UI built with egui / eframe
- Windows 11 Acrylic background via `window-vibrancy`
- Cross-crate split:
  * `imlag-core` — config, corpus, CFG generator, key/clipboard automation
  * `imlag-gui`  — desktop application
- GSI integration via the [`cs2-gsi`](https://github.com/ccc007ccc/cs2-gsi) crate
  (auto-writes `gamestate_integration_*.cfg` on first launch)

[Unreleased]: https://github.com/ccc007ccc/imlag-rs/commits/main
