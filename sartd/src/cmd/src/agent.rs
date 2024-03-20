use clap::Parser;
use sartd_kubernetes::{agent::{cni::server::CNI_SERVER_ENDPOINT, config::{DEFAULT_HTTPS_PORT, DEFAULT_HTTP_PORT}}, config::Mode};

#[derive(Debug, Clone, Parser)]
pub struct AgentCmd {
    #[arg(
        short,
        long,
        default_value = "127.0.0.5002",
        help = "Endpoint URL running Kubernetes agent"
    )]
    pub endpoint: String,

    #[arg(short = 'f', long, help = "Config file path for Agent daemon")]
    pub file: Option<String>,

    #[arg(long = "tls-cert", help = "path to TLS Certificate for agent")]
    pub tls_cert: Option<String>,

    #[arg(long = "tls-key", help = "path to TLS Key for agent")]
    pub tls_key: Option<String>,

    #[arg(long = "http-port", default_value_t = DEFAULT_HTTP_PORT, help = "HTTP server serving port")]
    pub http_port: u32,

    #[arg(long = "https-port", default_value_t = DEFAULT_HTTPS_PORT, help = "HTTPS server serving port")]
    pub https_port: u32,

    #[arg(
        long = "peer-state-watcher",
        help = "Endpoint URL for BGP peer state watcher"
    )]
    pub peer_state_watcher: Option<String>,

    #[arg(
        long = "mode",
        help = "Running mode(Default is Dual)",
        default_value_t = Mode::Dual,
    )]
    pub mode: Mode,

    #[arg(
        long = "cni-endpoint",
        default_value = CNI_SERVER_ENDPOINT,
        help = "Endpoint path running CNI server"
    )]
    pub cni_endpoint: String,
}
