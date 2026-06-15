# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

notalawyer embeds the license notices of a crate's dependencies at build time and prints them behind a `--license-notice` flag.

## Architecture

The three crates form a build-time → compile-time → runtime pipeline:

`notalawyer-build` runs in a consumer's `build.rs`, gathers dependency licenses through the **cargo-about library** (not the `cargo about` binary), and writes the NOTICE into `$OUT_DIR/notalawyer`. `notalawyer`'s `include_notice!` macro embeds that file as a `&'static str` at compile time. `notalawyer-clap` surfaces it behind a `--license-notice` flag. `example/` is the full wiring and doubles as the integration test.

## Gotchas

- `example/` is **excluded from the workspace**; build/run it from inside `example/`. Its `Cargo.lock` is committed (the root one is gitignored).
- `.cargo/config.toml` patches `notalawyer` to its local path, so the sibling crates build against the in-tree copy while `[workspace.dependencies]` pins the published version. `cargo publish --workspace` ignores the patch (harmless "patch not used" warning).
- `notalawyer-build` runs cargo-about offline by default; the default `fetch-clarify-license` feature adds a `ureq::Agent` for `[clarify.*.git]` remote license fetches. `default-features = false` keeps it fully offline (e.g. docs.rs).

## Release

Bump `[workspace.package].version` and `[workspace.dependencies].notalawyer` together, update `CHANGELOG.md`, tag `vX.Y.Z`, then `cargo publish --workspace` (publishes in dependency order). `release-dry-run.yml` runs `cargo publish --dry-run --workspace` on push/PR to catch publishability issues early.

## Conventions

- All changes go through PRs (no direct commits to `main`); call out breaking changes in the description.
- Develop test-first (TDD) — write the failing test/repro before the implementation.
- `cargo fmt` is applied as a separate step — don't mix formatting into logical commits.
