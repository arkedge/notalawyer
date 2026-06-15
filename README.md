# notalawyer

*NOTE: notalawyer does not provide legal advice, and the developers of notalawyer will not be held responsible for any events that occur as a result of using notalawyer.*

notalawyer embeds the license notices of your dependencies into your binary at build time and prints them at runtime behind a `--license-notice` flag. Licenses are gathered with [cargo-about](https://github.com/EmbarkStudios/cargo-about) used as a library, so the `cargo about` binary does not need to be installed.

It comes in three crates:

- **`notalawyer`** — the `include_notice!` macro that embeds the generated notice.
- **`notalawyer-clap`** — [clap](https://github.com/clap-rs/clap) integration: `ParseExt::parse_with_license_notice`, which adds the `--license-notice` flag (and re-exports `include_notice!`).
- **`notalawyer-build`** — the `build()` helper to call from `build.rs`.

## Usage

Add the crates (`notalawyer-build` is a build dependency):

```console
$ cargo add notalawyer notalawyer-clap
$ cargo add --build notalawyer-build
```

`about.toml` is [cargo-about's configuration file](https://embarkstudios.github.io/cargo-about/cli/generate/config.html) — notalawyer uses it as-is. Add one next to `Cargo.toml` listing the accepted licenses:

```toml
# about.toml
accepted = ["Apache-2.0", "MIT", "Unicode-DFS-2016"]
```

Call `notalawyer_build::build()` from `build.rs`:

```rust
// build.rs
fn main() {
    notalawyer_build::build();
}
```

Then parse your args with `parse_with_license_notice` instead of `parse`:

```rust
// src/main.rs
use clap::Parser;
use notalawyer_clap::*;

#[derive(Parser)]
struct Args { /* ... */ }

fn main() {
    let args = Args::parse_with_license_notice(include_notice!());
}
```

Running with `--license-notice` prints the notices and exits; otherwise parsing proceeds as usual. See the [`example/`](./example) directory for a complete, working setup.
