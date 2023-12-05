use clap::{Parser, Subcommand, ValueEnum};

use crate::{bgp::cmd::BgpCmd, fib::cmd::FibCmd};

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
pub(crate) struct Cmd {
    #[arg(
        value_enum,
        short = 'o',
        long,
        global = true,
        required = false,
        default_value = "plain",
        help = "Output format"
    )]
    pub output: Output,
    #[clap(subcommand)]
    pub sub: SubCmd,
}

#[derive(Debug, Clone, Subcommand)]
pub(crate) enum SubCmd {
    Bgp(BgpCmd),
    Fib(FibCmd),
}

#[derive(Debug, Clone, Parser, ValueEnum)]
pub(crate) enum Output {
    Plain,
    Json,
}
