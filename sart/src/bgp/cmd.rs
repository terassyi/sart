use clap::{Parser, Subcommand};

use super::{global::GlobalCmd, neighbor::NeighborCmd};

#[derive(Debug, Clone, Parser)]
pub(crate) struct BgpCmd {
    #[structopt(subcommand)]
    pub scope: Scope,
}

#[derive(Debug, Clone, Subcommand)]
pub(crate) enum Scope {
    Global(GlobalCmd),
    Neighbor(NeighborCmd),
}
