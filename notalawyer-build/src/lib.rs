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
    let client = reqwest::blocking::ClientBuilder::new().build().ok();

    let summary = cargo_about::licenses::Gatherer::with_store(Arc::new(store))
        .with_confidence_threshold(0.8)
        .with_max_depth(cfg.max_depth.map(|md| md as _))
        .gather(&krates, &cfg, client);

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

fn render_license_list(license_list: &cargo_about::generate::LicenseList<'_>) -> String {
    let mut output = String::new();
    for license in &license_list.licenses {
        output.push_str(&license.name);
        output.push_str("\n Used by:\n");
        for used_by in &license.used_by {
            output.push_str("  - ");
            output.push_str(&used_by.krate.name);
            output.push(' ');
            output.push_str(&used_by.krate.version.to_string());
            output.push_str(" (");
            if let Some(repo) = &used_by.krate.repository {
                output.push(' ');
                output.push_str(repo);
                output.push(' ');
            } else {
                output.push_str(" https://crates.io/crates/");
                output.push_str(&used_by.krate.name);
                output.push(' ');
            }
            output.push_str(")\n");
        }
        output.push('\n');
        output.push_str(&license.text);
        output.push_str(
            "\n--------------------------------------------------------------------------\n",
        );
    }
    output
}
