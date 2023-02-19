use clap::Parser;

#[derive(Debug, Clone, Parser)]
pub(crate) struct BgpCmd {
    #[arg(short = 'f', long, help = "Config file path for BGP daemon")]
    pub file: Option<String>,

    #[arg(short, long, help = "Local AS Number")]
    pub r#as: Option<u32>,

    #[arg(short, long, help = "Local router id(must be ipv4 format)")]
    pub router_id: Option<String>,

    #[arg(long = "fib", help = "Fib endpoint url(gRPC)")]
    pub fib_endpoint: Option<String>,

    #[arg(
        short,
        long,
        default_value = "info",
        help = "Log level(trace, debug, info, warn, error)"
    )]
    pub level: String,

    #[arg(short = 'o', long = "log-file", help = "Log output file path")]
    pub log_file: Option<String>,
}
