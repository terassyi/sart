use clap::Parser;
use sartd_kubernetes::controller::config::{DEFAULT_HTTPS_PORT, DEFAULT_HTTP_PORT};

#[derive(Debug, Clone, Parser)]
pub struct ControllerCmd {
    #[arg(
        short,
        long,
        default_value = "127.0.0.5003",
        help = "Endpoint URL running Kubernetes agent"
    )]
    pub endpoint: String,

    #[arg(long = "http-port", default_value_t = DEFAULT_HTTP_PORT, help = "HTTP server serving port")]
    pub http_port: u32,

    #[arg(long = "https-port", default_value_t = DEFAULT_HTTPS_PORT, help = "HTTPS server serving port")]
    pub https_port: u32,

    #[arg(short = 'f', long, help = "Config file path for Kubernetes controller")]
    pub file: Option<String>,

    #[arg(long = "tls-cert", help = "path to TLS Certificate for controller")]
    pub tls_cert: Option<String>,

    #[arg(long = "tls-key", help = "path to TLS Key for controller")]
    pub tls_key: Option<String>,
}
