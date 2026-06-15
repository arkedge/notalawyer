//! Build-time generation of dependency license notices.
//!
//! This crate is the build-script half of the `notalawyer` family. Called from
//! a crate's `build.rs`, [`build`] gathers the license texts of all
//! dependencies (using the [`cargo_about`] library) and writes a single
//! `NOTICE` file into `OUT_DIR`.
//!
//! The other two crates consume that file at compile time and runtime:
//!
//! - **`notalawyer-build`** (this crate) writes the `NOTICE` file from
//!   `build.rs`.
//! - [`notalawyer`](https://docs.rs/notalawyer) embeds it via
//!   `include_notice!`.
//! - [`notalawyer-clap`](https://docs.rs/notalawyer-clap) exposes it behind a
//!   `--license-notice` CLI flag.
//!
//! License acceptance and gathering are configured through an `about.toml`
//! file, searched for from the consuming crate's manifest directory upward;
//! see [cargo-about](https://embarkstudios.github.io/cargo-about/) for its
//! format.
//!
//! # Example
//!
//! Add `notalawyer-build` as a build dependency and call [`build`] from the
//! `main` of your `build.rs`:
//!
//! ```no_run
//! // build.rs
//! println!("cargo:rerun-if-changed=Cargo.toml");
//!
//! notalawyer_build::build();
//! ```

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Arc;

use cargo_about::licenses::config::{Config, KrateConfig};
use toml_span::Deserialize as _;

fn load_config(manifest_path: &camino::Utf8Path) -> Config {
    let mut parent = manifest_path.parent();

    while let Some(p) = parent {
        let about_toml = p.join("about.toml");

        if about_toml.exists() {
            if let Ok(contents) = std::fs::read_to_string(&about_toml) {
                // cargo-about 0.9 no longer derives serde `Deserialize` for its
                // config; it is parsed via `toml_span` and the hand-written
                // `Deserialize` impl instead.
                if let Ok(mut value) = toml_span::parse(&contents) {
                    if let Ok(cfg) = Config::deserialize(&mut value) {
                        return cfg;
                    }
                }
            }
        }

        parent = p.parent();
    }

    Config::default()
}

/// Gather dependency licenses and write the `NOTICE` file into `OUT_DIR`.
///
/// Intended to be called from a `build.rs`. It:
///
/// 1. Locates an `about.toml` config by walking up from the consuming crate's
///    `CARGO_MANIFEST_DIR` (falling back to defaults if none is found).
/// 2. Resolves the dependency graph and gathers each crate's license text via
///    the [`cargo_about`] library.
/// 3. Renders a combined notice and writes it to `$OUT_DIR/notalawyer`, the
///    path that [`notalawyer::include_notice!`](https://docs.rs/notalawyer)
///    later embeds.
///
/// It also emits `cargo:rerun-if-changed=about.toml` so the notice is
/// regenerated when the config changes.
///
/// # Panics
///
/// Panics if `OUT_DIR` or `CARGO_MANIFEST_DIR` are unset (i.e. when not run as
/// a build script), or if gathering, resolving, or writing the licenses fails.
///
/// # Example
///
/// ```no_run
/// // build.rs
/// println!("cargo:rerun-if-changed=Cargo.toml");
///
/// notalawyer_build::build();
/// ```
pub fn build() {
    let out_dir = std::env::var_os("OUT_DIR").unwrap();
    let license_path = Path::new(&out_dir).join("notalawyer");

    let manifest_path = std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR not set");
    let manifest_path = camino::Utf8PathBuf::from(manifest_path).join("Cargo.toml");

    let cfg = load_config(&manifest_path);

    let no_default_features = false;
    let all_features = false;
    let features: Vec<String> = vec![];
    let workspace = false;
    let target_overrides = &[];
    let krates = cargo_about::get_all_crates(
        &manifest_path,
        no_default_features,
        all_features,
        features,
        workspace,
        krates::LockOptions {
            frozen: false,
            locked: false,
            offline: false,
        },
        &cfg,
        target_overrides,
    )
    .expect("failed to gather crates");

    let store = cargo_about::licenses::store_from_cache().expect("failed to load license store");

    // cargo-about 0.9 takes an optional `ureq::Agent` for fetching remote
    // license information (clearlydefined.io, the original git repos for crates
    // that were packaged without their LICENSE files, ...). This is what backs
    // `[clarify.<crate>.git]` entries in `about.toml`.
    //
    // The `fetch-clarify-license` feature (enabled by default) decides whether
    // we hand cargo-about such an agent. When it is on we build the agent the
    // same way cargo-about's own CLI does, so the type unifies with what
    // `Gatherer::gather` expects. When it is off (`--no-default-features`) we
    // pass `None` and run fully offline -- handy for docs.rs or any sandboxed
    // build with no network access. Constructing the agent does not perform any
    // network access by itself; fetches only happen for `clarify.git` entries.
    #[cfg(feature = "fetch-clarify-license")]
    let client = {
        use ureq::tls::{RootCerts, TlsConfig};

        let prov = rustls::crypto::ring::default_provider();

        Some(
            ureq::Agent::config_builder()
                .tls_config(
                    TlsConfig::builder()
                        .unversioned_rustls_crypto_provider(Arc::new(prov))
                        .root_certs(RootCerts::PlatformVerifier)
                        .build(),
                )
                .build()
                .new_agent(),
        )
    };
    #[cfg(not(feature = "fetch-clarify-license"))]
    let client = None;

    let summary = cargo_about::licenses::Gatherer::with_store(Arc::new(store))
        .with_confidence_threshold(0.8)
        .with_max_depth(cfg.max_depth.map(|md| md as _))
        .gather(&krates, &cfg, client);

    // `resolve` no longer returns the codespan `Files`; it now takes a mutable
    // reference to one (which it appends synthesized manifests to) and the
    // per-crate config as a plain `BTreeMap<String, KrateConfig>`.
    let krate_cfg: BTreeMap<String, KrateConfig> = cfg
        .crates
        .into_iter()
        .map(|(name, spanned)| (name, spanned.value))
        .collect();

    let mut files = cargo_about::licenses::resolution::Files::new();
    let fail_on_missing = false;
    let resolved = cargo_about::licenses::resolution::resolve(
        &summary,
        &cfg.accepted,
        &krate_cfg,
        &mut files,
        fail_on_missing,
    );

    // `generate` now reports diagnostics through a callback instead of taking a
    // termcolor stream. Forward them to stderr so license validation problems
    // are still surfaced during the build.
    use codespan_reporting::term;
    let stream = term::termcolor::StandardStream::stderr(term::termcolor::ColorChoice::Auto);
    let diag_cfg = term::Config::default();
    let license_list = cargo_about::generate::generate(&summary, &resolved, |diags| {
        let mut stream = stream.lock();
        for diag in diags {
            let _ = term::emit_to_io_write(&mut stream, &diag_cfg, &files, diag);
        }
    })
    .expect("failed to generate license list");

    let output = render_license_list(&license_list);

    std::fs::write(&license_path, output).expect("failed to write NOTICE file");

    println!("cargo:rerun-if-changed=about.toml");
}

/// Renders the parenthesised link shown after each crate in the "Used by" list.
///
/// Prefers the crate's `repository`, falling back to its crates.io page. The
/// surrounding spaces are intentional, kept to match the original `about.hbs`
/// output byte-for-byte.
fn crate_link(name: &str, repository: Option<&str>) -> String {
    match repository {
        Some(repo) => format!(" {repo} "),
        None => format!(" https://crates.io/crates/{name} "),
    }
}

fn render_license_list(license_list: &cargo_about::generate::LicenseList<'_>) -> String {
    use std::fmt::Write as _;

    let mut output = String::new();
    for license in &license_list.licenses {
        writeln!(output, "{}\n Used by:", license.name).unwrap();
        for used_by in &license.used_by {
            let krate = &used_by.krate;
            let link = crate_link(&krate.name, krate.repository.as_deref());
            writeln!(output, "  - {} {} ({link})", krate.name, krate.version).unwrap();
        }
        writeln!(
            output,
            "\n{}\n--------------------------------------------------------------------------",
            license.text
        )
        .unwrap();
    }
    output
}

#[cfg(test)]
mod tests {
    use super::crate_link;

    #[test]
    fn crate_link_uses_repository_when_present() {
        assert_eq!(
            crate_link("serde", Some("https://github.com/serde-rs/serde")),
            " https://github.com/serde-rs/serde "
        );
    }

    #[test]
    fn crate_link_falls_back_to_crates_io_page() {
        assert_eq!(
            crate_link("serde", None),
            " https://crates.io/crates/serde "
        );
    }
}
