//! Embed and access the license notices of a crate's dependencies.
//!
//! `notalawyer` is the runtime core of a three-crate family that lets a binary
//! display the license notices of every crate it depends on:
//!
//! - [`notalawyer-build`](https://docs.rs/notalawyer-build) runs from a
//!   `build.rs`, gathers the dependency licenses, and writes a `NOTICE` file
//!   into `OUT_DIR`.
//! - **`notalawyer`** (this crate) provides the [`include_notice!`] macro, which
//!   embeds that generated `NOTICE` file into the binary as a `&'static str`.
//! - [`notalawyer-clap`](https://docs.rs/notalawyer-clap) wires the notice into
//!   a [`clap`](https://docs.rs/clap) CLI behind a `--license-notice` flag.
//!
//! This crate has no dependencies and exposes only the [`include_notice!`]
//! macro.
//!
//! # Example
//!
//! ```ignore
//! // The build script (notalawyer-build) wrote a NOTICE file into OUT_DIR.
//! // `include_notice!` embeds it as a `&'static str` at compile time.
//! let notice: &'static str = notalawyer::include_notice!();
//! print!("{notice}");
//! ```

/// Embed the generated dependency-license `NOTICE` file as a `&'static str`.
///
/// This macro expands to an [`include_str!`] of the `NOTICE` file that
/// [`notalawyer-build`](https://docs.rs/notalawyer-build) writes into `OUT_DIR`
/// at build time. Because it reads `OUT_DIR`, it only compiles in a crate whose
/// `build.rs` calls `notalawyer_build::build()`.
///
/// The expansion is purely a compile-time `include_str!`, so the notice text is
/// baked into the binary with no runtime file access.
///
/// # Example
///
/// Pair it with [`notalawyer-clap`](https://docs.rs/notalawyer-clap) to expose
/// the notice through a CLI flag:
///
/// ```ignore
/// use clap::Parser;
/// use notalawyer_clap::*;
///
/// #[derive(Parser)]
/// struct Args {
///     #[clap(long)]
///     flag: bool,
/// }
///
/// fn main() {
///     // Running the binary with `--license-notice` prints the embedded notice.
///     let _args = Args::parse_with_license_notice(include_notice!());
/// }
/// ```
#[macro_export]
macro_rules! include_notice {
    () => {
        include_str!(concat!(env!("OUT_DIR"), concat!("/notalawyer")));
    };
}
