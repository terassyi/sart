use clap::Parser;

#[derive(Debug, Clone, Parser)]
pub(crate) struct AgentCmd {

	#[arg(
		short,
		long,
		default_value = "127.0.0.5002",
		help = "Endpoint URL running Kubernetes agent"
	)]
	pub endpoint: String,

	#[arg(short = 'f', long, help = "Config file path for Agent daemon")]
	pub file: Option<String>,

	#[arg(long="tls-cert", help = "path to TLS Certificate for agent")]
	pub tls_cert: Option<String>,

	#[arg(long="tls-key", help = "path to TLS Key for agent")]
	pub tls_key: Option<String>,
}
