use clap::{Parser, Subcommand};

#[derive(Debug, Clone, Parser)]
pub(crate) struct FibCmd {
    #[structopt(subcommand)]
    pub action: Action,
}

#[derive(Debug, Clone, Subcommand)]
pub(crate) enum Action {
    Get,
    Add,
    Delete,
}
