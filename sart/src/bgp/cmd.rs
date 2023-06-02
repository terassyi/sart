use clap::{Parser, Subcommand};

use super::{global::GlobalCmd, neighbor::NeighborCmd};

#[derive(Debug, Clone, Parser)]
pub(crate) struct BgpCmd {
    #[structopt(subcommand)]
    pub scope: Scope,

    #[arg(
        short = 'e',
        long,
        global = true,
        required = false,
        default_value = "localhost:5000",
        help = "Endpoint to BGP API server"
    )]
    pub endpoint: String,
}

#[derive(Debug, Clone, Subcommand)]
pub(crate) enum Scope {
    Global(GlobalCmd),
    Neighbor(NeighborCmd),
}
