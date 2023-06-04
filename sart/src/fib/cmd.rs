use clap::{Parser, Subcommand};

use super::channel::ChannelCmd;

#[derive(Debug, Clone, Parser)]
pub(crate) struct FibCmd {
    #[structopt(subcommand)]
    pub scope: Scope,


    #[arg(
        short = 'e',
        long,
        global = true,
        required = false,
        default_value = "localhost:5001",
        help = "Endpoint to FIB API server"
    )]
    pub endpoint: String,
}

#[derive(Debug, Clone, Subcommand)]
pub(crate) enum Scope {
    Channel(ChannelCmd),
}
