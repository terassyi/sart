use clap::Parser;

#[derive(Debug, Clone, Parser)]
pub struct BgpCmd {
    #[arg(short = 'f', long, help = "Config file path for BGP daemon")]
    pub file: Option<String>,

    #[arg(short, long, help = "Local AS Number")]
    pub r#as: Option<u32>,

    #[arg(short, long, help = "Local router id(must be ipv4 format)")]
    pub router_id: Option<String>,

    #[arg(long = "fib", help = "Fib endpoint url(gRPC) exp) localhost:5001")]
    pub fib_endpoint: Option<String>,

    #[arg(long = "table-id", help = "Target fib table id(default is main(254))")]
    pub fib_table_id: Option<u8>,

    #[arg(long = "exporter", help = "Exporter endpoint url")]
    pub exporter: Option<String>,

    #[arg(
        short,
        long,
        default_value = "info",
        help = "Log level(trace, debug, info, warn, error)"
    )]
    pub level: String,
}
