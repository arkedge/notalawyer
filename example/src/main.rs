use clap::Parser;
use notalawyer_clap::*;

#[derive(Parser)]
struct Args {
    #[clap(long)]
    flag: bool,
}

fn main() {
    Args::parse_with_license_notice(include_notice!());
}
