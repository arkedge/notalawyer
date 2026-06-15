# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## Unreleased

## [0.3.0](https://github.com/arkedge/notalawyer/releases/tag/v0.3.0) - 2026-06-16

### Changed

- `notalawyer-build` now generates the license notice via the cargo-about
  **library** (0.9) instead of shelling out to the `cargo about` binary, so the
  binary no longer needs to be installed — cargo-about and its dependencies are
  pulled in as build-dependencies instead
  ([#8](https://github.com/arkedge/notalawyer/pull/8), [#23](https://github.com/arkedge/notalawyer/pull/23)).
- The minimum supported Rust version is now **1.88** (required by cargo-about
  0.9) ([#25](https://github.com/arkedge/notalawyer/pull/25)).
- **Breaking:** custom notice output changed from a handlebars template
  (`build_with_template`, removed along with `about_hbs`) to the renderer
  closures `build_with` / `try_build_with`
  ([#8](https://github.com/arkedge/notalawyer/pull/8), [#29](https://github.com/arkedge/notalawyer/pull/29)).

### Added

- A default-enabled `fetch-clarify-license` feature on `notalawyer-build`.
  Disable it (`default-features = false`) for a fully offline build — e.g. on
  docs.rs or other sandboxed environments
  ([#24](https://github.com/arkedge/notalawyer/pull/24)).
- `notalawyer_build::gather`, `build_with`, and `try_build_with` for rendering
  the notice in a custom format, exposing owned `Serialize` types (`Notice`,
  `License`, `Package`, …). `build()`'s default output is unchanged
  ([#29](https://github.com/arkedge/notalawyer/pull/29)).

### Fixed

- The build script no longer panics under PowerShell / on Windows. cargo-about
  0.9.0+ rejected the previous piped-stdout invocation; calling the library
  directly avoids it entirely
  ([#9](https://github.com/arkedge/notalawyer/issues/9)).

## [0.2.0](https://github.com/arkedge/notalawyer/releases/tag/v0.2.0) - 2024-11-18

### Fixed

- Build-cache invalidation: `notalawyer-build` no longer emits an incorrect
  `cargo:rerun-if-changed=Cargo.lock`. Consumers should emit
  `cargo:rerun-if-changed=Cargo.toml` from their `build.rs` (as `example/` does)
  so the notice regenerates when dependencies change.

## [0.1.0](https://github.com/arkedge/notalawyer/releases/tag/v0.1.0) - 2024-02-14

- Initial release.
