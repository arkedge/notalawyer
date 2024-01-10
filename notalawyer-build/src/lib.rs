use std::path::{Path, PathBuf};

pub fn about_hbs() -> Option<PathBuf> {
    Some(PathBuf::from(file!()).parent()?.parent()?.join("about.hbs"))
}

pub fn build() {
    let about_hbs = about_hbs().expect("failed to traverse to about.hbs");
    build_with_template(about_hbs.to_str().expect("invalid about.hbs path"));
}

pub fn build_with_template(template: &str) {
    let out_dir = std::env::var_os("OUT_DIR").unwrap();
    let license_path = Path::new(&out_dir).join("notalawyer");
    let mut writer = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(&license_path)
        .expect("failed to open NOTICE file");
    let mut child = std::process::Command::new("cargo")
        .args(&["about", "generate", template])
        .stdout(std::process::Stdio::piped())
        .spawn()
        .expect("failed to run cargo-about");
    let mut stdout = child.stdout.take().unwrap();
    std::io::copy(&mut stdout, &mut writer).expect("failed to write NOTICE file");
    let is_success = child
        .wait()
        .expect("failed to wait for cargo-about")
        .success();
    if !is_success {
        panic!("cargo-about failed");
    }
    println!("cargo:rerun-if-changed=Cargo.lock");
    println!("cargo:rerun-if-changed={}", template);
}
