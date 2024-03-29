use clap::Parser;

#[derive(Debug, Clone, Parser)]
pub struct FibCmd {
    #[arg(
        short,
        long,
        default_value = "127.0.0.1:5001",
        help = "Fib manager running endpoint url"
    )]
    pub endpoint: String,

    #[arg(
        short = 'f',
        long,
        required = true,
        help = "Config file path for Fib daemon"
    )]
    pub file: Option<String>,
}
