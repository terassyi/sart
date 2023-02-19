use clap::{Arg, Parser, Subcommand, ValueEnum};

use crate::{bgp::cmd::BgpCmd, fib::FibCmd};

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
pub(crate) struct Cmd {
    #[arg(
        value_enum,
        short = 'd',
        long,
        global = true,
        required = false,
        default_value = "plain",
        help = "Display format"
    )]
    pub format: Format,
    #[arg(
        short = 'e',
        long,
        global = true,
        required = false,
        default_value = "localhost:5000",
        help = "Endpoint to API server"
    )]
    pub endpoint: String,
    #[clap(subcommand)]
    pub sub: SubCmd,
}

#[derive(Debug, Clone, Subcommand)]
pub(crate) enum SubCmd {
    Bgp(BgpCmd),
    Fib(FibCmd),
}

#[derive(Debug, Clone, Parser, ValueEnum)]
pub(crate) enum Format {
    Plain,
    Json,
}
