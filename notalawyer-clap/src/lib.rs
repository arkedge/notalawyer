//! [`clap`] integration for displaying dependency license notices.
//!
//! This crate is the CLI-facing layer of the `notalawyer` family. It adds a
//! `--license-notice` flag to any [`clap::Parser`] command via the [`ParseExt`]
//! extension trait, printing the embedded notice and exiting when the flag is
//! present.
//!
//! It works together with the other two crates:
//!
//! - [`notalawyer-build`](https://docs.rs/notalawyer-build) generates the
//!   `NOTICE` file from a `build.rs`.
//! - [`notalawyer`](https://docs.rs/notalawyer) embeds it via
//!   [`include_notice!`], which is re-exported here for convenience.
//! - **`notalawyer-clap`** (this crate) feeds that string to
//!   [`ParseExt::parse_with_license_notice`].
//!
//! # Example
//!
//! ```ignore
//! use clap::Parser;
//! use notalawyer_clap::*;
//!
//! #[derive(Parser)]
//! struct Args {
//!     #[clap(long)]
//!     flag: bool,
//! }
//!
//! fn main() {
//!     // `include_notice!()` provides the embedded `&'static str`; the helper
//!     // adds a `--license-notice` flag that prints it and exits.
//!     let _args = Args::parse_with_license_notice(include_notice!());
//! }
//! ```

/// Re-export of [`notalawyer::include_notice!`] so callers can depend on this
/// crate alone.
///
/// Use it to produce the `&str` argument for
/// [`ParseExt::parse_with_license_notice`]:
///
/// ```ignore
/// use notalawyer_clap::include_notice;
///
/// let notice: &'static str = include_notice!();
/// ```
pub use notalawyer::include_notice;

/// Extension trait that adds a `--license-notice` flag to any [`clap::Parser`].
///
/// A blanket implementation covers every type that implements
/// [`clap::Parser`], so deriving `Parser` is enough to gain
/// [`parse_with_license_notice`](ParseExt::parse_with_license_notice).
///
/// # Example
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
/// // `Args` automatically gains `parse_with_license_notice` via the blanket impl.
/// let _args = Args::parse_with_license_notice(include_notice!());
/// ```
pub trait ParseExt: clap::Parser {
    /// Parse the process arguments, adding a `--license-notice` flag.
    ///
    /// This behaves like [`clap::Parser::parse`], except that the command is
    /// augmented with a `--license-notice` flag. When that flag is passed,
    /// `notice` is printed to stdout and the process exits with status `0`
    /// before any of the parser's own fields are produced. Otherwise the
    /// arguments are parsed and the resulting value is returned (errors are
    /// formatted and reported through clap's usual exit path).
    ///
    /// `notice` is typically produced by [`include_notice!`].
    ///
    /// # Example
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
    /// // With `--license-notice`, this prints the notice and exits;
    /// // otherwise it returns the parsed `Args`.
    /// let _args = Args::parse_with_license_notice(include_notice!());
    /// ```
    fn parse_with_license_notice(notice: &str) -> Self {
        fn patch_command(cmd: clap::Command) -> clap::Command {
            cmd.arg(clap::arg!(--"license-notice" "Show license notices"))
        }

        let matches = patch_command(Self::command_for_update()).get_matches();
        if matches.get_flag("license-notice") {
            print!("{notice}");
            std::process::exit(0);
        }
        // For better error message, we need to use build Command twice.
        let mut command = patch_command(Self::command());
        let mut matches = command.clone().get_matches();
        Self::from_arg_matches_mut(&mut matches)
            .map_err(|err| err.format(&mut command))
            .unwrap_or_else(|err| err.exit())
    }
}

impl<T: clap::Parser> ParseExt for T {}
