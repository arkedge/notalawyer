use std::path::{Path, PathBuf};
use std::sync::Arc;

pub fn about_hbs() -> Option<PathBuf> {
    Some(PathBuf::from(file!()).parent()?.parent()?.join("about.hbs"))
}

fn load_config(manifest_path: &camino::Utf8Path) -> cargo_about::licenses::config::Config {
    let mut parent = manifest_path.parent();

    // Move up directories until we find an about.toml
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
    let about_hbs = about_hbs().expect("failed to traverse to about.hbs");
    build_with_template(about_hbs.to_str().expect("invalid about.hbs path"));
}

pub fn build_with_template(template: &str) {
    let out_dir = std::env::var_os("OUT_DIR").unwrap();
    let license_path = Path::new(&out_dir).join("notalawyer");

    // Find Cargo.toml in current directory
    let manifest_path = std::env::var("CARGO_MANIFEST_DIR")
        .expect("CARGO_MANIFEST_DIR not set");
    let manifest_path = camino::Utf8PathBuf::from(manifest_path).join("Cargo.toml");

    // Load config from about.toml if it exists
    let cfg = load_config(&manifest_path);

    // Get all crates
    let krates = cargo_about::get_all_crates(
        &manifest_path,
        false, // no_default_features
        false, // all_features
        vec![], // features
        false, // workspace
        krates::LockOptions {
            frozen: false,
            locked: false,
            offline: false,
        },
        &cfg,
        &[], // target_overrides
    )
    .expect("failed to gather crates");

    // Load license store
    let store = cargo_about::licenses::store_from_cache()
        .expect("failed to load license store");

    // Create HTTP client for fetching license information from remote sources
    // (unless running in offline mode would be specified via about.toml or environment)
    let client = reqwest::blocking::ClientBuilder::new()
        .build()
        .ok();

    // Gather license information
    let summary = cargo_about::licenses::Gatherer::with_store(Arc::new(store))
        .with_confidence_threshold(0.8)
        .with_max_depth(cfg.max_depth.map(|md| md as _))
        .gather(&krates, &cfg, client);

    // Resolve licenses
    let (files, resolved) = cargo_about::licenses::resolution::resolve(
        &summary,
        &cfg.accepted,
        &cfg.crates,
        false, // fail_on_missing
    );

    // Generate license list
    // Pass stderr stream to enable license validation errors
    use codespan_reporting::term;
    let stream = term::termcolor::StandardStream::stderr(term::termcolor::ColorChoice::Auto);
    let input = cargo_about::generate::generate(&summary, &resolved, &files, Some(stream))
        .expect("failed to generate license list");

    // Render with handlebars
    let mut reg = handlebars::Handlebars::new();
    reg.register_template_file("tmpl", template)
        .expect("failed to register template");

    let output = reg.render("tmpl", &input)
        .expect("failed to render template");

    // Write output
    std::fs::write(&license_path, output)
        .expect("failed to write NOTICE file");

    println!("cargo:rerun-if-changed={}", template);
}
