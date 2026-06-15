//! Build-time generation of dependency license notices.
//!
//! This crate is the build-script half of the `notalawyer` family. Called from
//! a crate's `build.rs`, it gathers the license texts of all dependencies
//! (using the [`cargo_about`] library) and writes a single `NOTICE` file into
//! `OUT_DIR`.
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
//! # Choosing a renderer
//!
//! There are three entry points, in increasing order of control:
//!
//! - [`build`] writes the built-in default format. This is all most crates
//!   need.
//! - [`build_with`] gathers the data, hands you a [`Notice`], and writes
//!   whatever `String` your closure returns.
//! - [`try_build_with`] is the fallible variant for renderers (templating
//!   engines, etc.) that can fail.
//!
//! [`gather`] is the lowest level: it returns a [`Notice`] and writes nothing,
//! so you can inspect or serialize the data yourself. The [`Notice`],
//! [`License`], [`UsedBy`] and [`Package`] types are owned and implement
//! [`serde::Serialize`], so you can feed them straight into your own
//! templating engine without any of `cargo-about`'s lifetime-bound types
//! leaking into your build script.
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
//!
//! To customize the output, render a [`Notice`] yourself:
//!
//! ```no_run
//! // build.rs
//! use std::fmt::Write as _;
//!
//! notalawyer_build::build_with(|notice| {
//!     let mut out = String::new();
//!     for license in &notice.licenses {
//!         writeln!(out, "## {} ({})", license.name, license.id).unwrap();
//!         out.push_str(&license.text);
//!     }
//!     out
//! });
//! ```

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Arc;

use cargo_about::licenses::config::{Config, KrateConfig};
use toml_span::Deserialize as _;

/// A complete license notice: every distinct license text and the flat list of
/// crates it was gathered from.
///
/// This is the owned, [`Serialize`](serde::Serialize)-able counterpart of
/// `cargo-about`'s internal license list. It carries no borrows, so you are
/// free to store it, serialize it, or hand it to your own templating engine.
#[non_exhaustive]
#[derive(Debug, Clone, serde::Serialize)]
pub struct Notice {
    /// Every distinct license *text*, each with the crates that use it. Two
    /// crates under the same SPDX license may appear under different entries if
    /// their license texts differ (e.g. a different copyright line).
    pub licenses: Vec<License>,
    /// All crates considered, regardless of license. This mirrors
    /// `cargo-about`'s flat `crates` array.
    pub crates: Vec<Package>,
}

/// A single license text and the crates that use it.
#[non_exhaustive]
#[derive(Debug, Clone, serde::Serialize)]
pub struct License {
    /// The SPDX short identifier for the license, e.g. `"Apache-2.0"`.
    pub id: String,
    /// The full, human-readable name of the license, e.g. `"Apache License
    /// 2.0"`.
    pub name: String,
    /// The full license text.
    pub text: String,
    /// The crates this particular license text was applied to.
    pub used_by: Vec<UsedBy>,
}

/// A single "crate uses this license" entry.
#[non_exhaustive]
#[derive(Debug, Clone, serde::Serialize)]
pub struct UsedBy {
    /// The crate that uses the enclosing [`License`]. Serialized under the key
    /// `"crate"` (since `crate` is a reserved word in Rust).
    #[serde(rename = "crate")]
    pub krate: Package,
}

/// An owned description of a single crate/package.
#[non_exhaustive]
#[derive(Debug, Clone, serde::Serialize)]
pub struct Package {
    /// `cargo-metadata`'s opaque package id (its `PackageId` string
    /// representation). Stable enough to disambiguate two versions of the same
    /// crate, but not a clean human-facing value -- prefer [`name`](Self::name)
    /// and [`version`](Self::version) for display.
    pub id: String,
    /// The crate's name as given in its `Cargo.toml`.
    pub name: String,
    /// The crate's version, formatted as a string (e.g. `"1.2.3"`).
    pub version: String,
    /// The crate's `repository` URL, if it declared one.
    pub repository: Option<String>,
}

/// An error from [`try_build_with`].
///
/// Wraps either a failure while gathering/writing the notice, or an error
/// returned by the user-supplied renderer.
#[derive(Debug)]
#[non_exhaustive]
pub enum Error {
    /// The user-supplied renderer returned an error.
    Render(Box<dyn std::error::Error + Send + Sync>),
    /// Writing the rendered notice to `$OUT_DIR/notalawyer` failed.
    Write(std::io::Error),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Render(e) => write!(f, "failed to render license notice: {e}"),
            Error::Write(e) => write!(f, "failed to write NOTICE file: {e}"),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Error::Render(e) => Some(&**e),
            Error::Write(e) => Some(e),
        }
    }
}

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

/// Map a `cargo-about`/`cargo-metadata` package into our owned [`Package`].
fn to_package(pkg: &krates::cm::Package) -> Package {
    Package {
        id: pkg.id.repr.clone(),
        name: pkg.name.clone(),
        version: pkg.version.to_string(),
        repository: pkg.repository.clone(),
    }
}

/// Gather dependency license data without writing anything.
///
/// This performs all the work [`build`] does up to (but not including) writing
/// the `NOTICE` file: it locates an `about.toml`, resolves the dependency
/// graph, and gathers each crate's license text via the [`cargo_about`]
/// library, returning the result as an owned [`Notice`].
///
/// Use this when you want to inspect or serialize the data yourself; use
/// [`build_with`] / [`try_build_with`] when you also want it written to
/// `$OUT_DIR/notalawyer`.
///
/// Unlike [`build`], this does **not** emit `cargo:rerun-if-changed`; the
/// `build_*` helpers do.
///
/// # Panics
///
/// Panics if `CARGO_MANIFEST_DIR` is unset (i.e. when not run as a build
/// script), or if gathering or resolving the licenses fails.
pub fn gather() -> Notice {
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

    // Map cargo-about's lifetime-bound license list into our owned `Notice`.
    let licenses = license_list
        .licenses
        .iter()
        .map(|license| License {
            id: license.id.clone(),
            name: license.name.clone(),
            text: license.text.clone(),
            used_by: license
                .used_by
                .iter()
                .map(|used_by| UsedBy {
                    krate: to_package(used_by.krate),
                })
                .collect(),
        })
        .collect();

    let crates = license_list
        .crates
        .iter()
        .map(|pl| to_package(pl.package))
        .collect();

    Notice { licenses, crates }
}

/// Gather licenses, render them with `render`, and write the result to
/// `$OUT_DIR/notalawyer`.
///
/// This is the customizable counterpart of [`build`]: it does everything
/// [`build`] does, but lets you decide how the [`Notice`] turns into text.
///
/// Emits `cargo:rerun-if-changed=about.toml`.
///
/// # Panics
///
/// Panics if `OUT_DIR`/`CARGO_MANIFEST_DIR` are unset, if gathering fails (see
/// [`gather`]), or if writing the notice fails. Use [`try_build_with`] for a
/// fallible renderer.
///
/// # Example
///
/// ```no_run
/// // build.rs
/// notalawyer_build::build_with(|notice| {
///     notice
///         .licenses
///         .iter()
///         .map(|l| format!("{}\n{}\n", l.name, l.text))
///         .collect()
/// });
/// ```
pub fn build_with(render: impl FnOnce(&Notice) -> String) {
    let notice = gather();
    let output = render(&notice);
    write_notice(&output).expect("failed to write NOTICE file");
    println!("cargo:rerun-if-changed=about.toml");
}

/// Fallible variant of [`build_with`] for renderers that can fail (templating
/// engines, etc.).
///
/// Gathers licenses, renders them with `render`, and writes the result to
/// `$OUT_DIR/notalawyer`. A renderer error is wrapped in [`Error::Render`] and
/// a write failure in [`Error::Write`].
///
/// Emits `cargo:rerun-if-changed=about.toml`.
///
/// # Panics
///
/// Panics if `OUT_DIR`/`CARGO_MANIFEST_DIR` are unset or if gathering fails
/// (see [`gather`]). Renderer and write failures are returned as [`Error`]
/// rather than panicking.
///
/// # Example
///
/// ```no_run
/// // build.rs
/// notalawyer_build::try_build_with(|notice| {
///     // your templating engine here; this stub never fails
///     Ok::<_, std::convert::Infallible>(format!("{} licenses", notice.licenses.len()))
/// })
/// .unwrap();
/// ```
pub fn try_build_with<E>(render: impl FnOnce(&Notice) -> Result<String, E>) -> Result<(), Error>
where
    E: std::error::Error + Send + Sync + 'static,
{
    let notice = gather();
    let output = render(&notice).map_err(|e| Error::Render(Box::new(e)))?;
    write_notice(&output).map_err(Error::Write)?;
    println!("cargo:rerun-if-changed=about.toml");
    Ok(())
}

/// Gather dependency licenses and write the `NOTICE` file into `OUT_DIR` using
/// the built-in default format.
///
/// Intended to be called from a `build.rs`. It:
///
/// 1. Locates an `about.toml` config by walking up from the consuming crate's
///    `CARGO_MANIFEST_DIR` (falling back to defaults if none is found).
/// 2. Resolves the dependency graph and gathers each crate's license text via
///    the [`cargo_about`] library (see [`gather`]).
/// 3. Renders a combined notice with the default format and writes it to
///    `$OUT_DIR/notalawyer`, the path that
///    [`notalawyer::include_notice!`](https://docs.rs/notalawyer) later embeds.
///
/// It also emits `cargo:rerun-if-changed=about.toml` so the notice is
/// regenerated when the config changes.
///
/// This is exactly [`build_with`] called with the default renderer; reach for
/// [`build_with`] / [`try_build_with`] / [`gather`] to customize the output.
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
    build_with(render_default);
}

/// Write `output` to `$OUT_DIR/notalawyer`.
fn write_notice(output: &str) -> std::io::Result<()> {
    let out_dir = std::env::var_os("OUT_DIR").expect("OUT_DIR not set");
    let license_path = Path::new(&out_dir).join("notalawyer");
    std::fs::write(&license_path, output)
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

/// The built-in default notice format.
///
/// Byte-identical to the format `notalawyer` has always produced; locked by the
/// `render_default_matches_golden` test.
fn render_default(notice: &Notice) -> String {
    use std::fmt::Write as _;

    let mut output = String::new();
    for license in &notice.licenses {
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
    use super::{crate_link, render_default, License, Notice, Package, UsedBy};

    fn pkg(name: &str, version: &str, repository: Option<&str>) -> Package {
        Package {
            id: format!("{name} {version} (registry+...)"),
            name: name.to_owned(),
            version: version.to_owned(),
            repository: repository.map(str::to_owned),
        }
    }

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

    /// The highest-value test: the default renderer must stay byte-identical to
    /// what `notalawyer` has always emitted. Build a `Notice` by hand (our types
    /// are constructible, unlike cargo-about's) covering both link branches and
    /// assert the exact output.
    #[test]
    fn render_default_matches_golden() {
        let notice = Notice {
            licenses: vec![
                License {
                    id: "MIT".to_owned(),
                    name: "MIT License".to_owned(),
                    text: "MIT license text".to_owned(),
                    used_by: vec![UsedBy {
                        krate: pkg("foo", "1.0.0", Some("https://github.com/example/foo")),
                    }],
                },
                License {
                    id: "Apache-2.0".to_owned(),
                    name: "Apache License 2.0".to_owned(),
                    text: "Apache license text".to_owned(),
                    used_by: vec![UsedBy {
                        krate: pkg("bar", "2.3.4", None),
                    }],
                },
            ],
            crates: vec![],
        };

        let dashes = "-".repeat(74);
        let expected = format!(
            "MIT License\n Used by:\n  - foo 1.0.0 ( https://github.com/example/foo )\n\nMIT license text\n{dashes}\nApache License 2.0\n Used by:\n  - bar 2.3.4 ( https://crates.io/crates/bar )\n\nApache license text\n{dashes}\n"
        );

        assert_eq!(render_default(&notice), expected);
    }

    /// The `krate` field must serialize under the key `"crate"`, not `"krate"`.
    #[test]
    fn used_by_serializes_krate_as_crate() {
        let used_by = UsedBy {
            krate: pkg("foo", "1.0.0", None),
        };
        let value = serde_json::to_value(&used_by).unwrap();
        let obj = value.as_object().unwrap();
        assert!(obj.contains_key("crate"), "expected key `crate`: {value}");
        assert!(!obj.contains_key("krate"), "did not expect key `krate`");
        assert_eq!(value["crate"]["name"], "foo");
        assert_eq!(value["crate"]["version"], "1.0.0");
    }
}
