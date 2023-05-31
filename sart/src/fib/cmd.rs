use clap::{Parser, Subcommand};

use super::channel::ChannelCmd;

#[derive(Debug, Clone, Parser)]
pub(crate) struct FibCmd {
    #[structopt(subcommand)]
    pub scope: Scope,
}

#[derive(Debug, Clone, Subcommand)]
pub(crate) enum Scope {
    Channel(ChannelCmd),
}
