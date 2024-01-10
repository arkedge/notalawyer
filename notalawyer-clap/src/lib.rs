pub use notalawyer::include_notice;

pub trait ParseExt: clap::Parser {
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
