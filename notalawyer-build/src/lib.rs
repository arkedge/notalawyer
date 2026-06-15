use std::path::Path;
use std::sync::Arc;

fn load_config(manifest_path: &camino::Utf8Path) -> cargo_about::licenses::config::Config {
    let mut parent = manifest_path.parent();

    while let Some(p) = parent {
        let about_toml = p.join("about.toml");

        if about_toml.exists() {
            if let Ok(contents) = std::fs::read_to_string(&about_toml) {
                if let Ok(cfg) = toml::from_str(&contents) {
                    return cfg;
                }
            }
        }

        parent = p.parent();
    }

    cargo_about::licenses::config::Config::default()
}

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

    // Create HTTP client for fetching license information from remote sources
    let client = reqwest::blocking::ClientBuilder::new()
        .build()
        .expect("failed to create HTTP client");

    let summary = cargo_about::licenses::Gatherer::with_store(Arc::new(store))
        .with_confidence_threshold(0.8)
        .with_max_depth(cfg.max_depth.map(|md| md as _))
        .gather(&krates, &cfg, Some(client));

    let fail_on_missing = false;
    let (files, resolved) = cargo_about::licenses::resolution::resolve(
        &summary,
        &cfg.accepted,
        &cfg.crates,
        fail_on_missing,
    );

    // Pass stderr stream to enable license validation errors
    use codespan_reporting::term;
    let stream = term::termcolor::StandardStream::stderr(term::termcolor::ColorChoice::Auto);
    let license_list = cargo_about::generate::generate(&summary, &resolved, &files, Some(stream))
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
