use clap::Parser;

#[derive(Debug, Clone, Parser)]
pub(crate) struct FibCmd {
    #[arg(
        short,
        long,
        default_value = "localhost:5001",
        help = "Fib manager running endpoint url"
    )]
    pub endpoint: String,
}
