use clap::Parser;

#[derive(Debug, Clone, Parser)]
pub(crate) struct ControllerCmd {

	#[arg(
		short,
		long,
		default_value = "127.0.0.5003",
		help = "Endpoint URL running Kubernetes agent"
	)]
	pub endpoint: String,

	#[arg(short = 'f', long, help = "Config file path for Kubernetes controller")]
	pub file: Option<String>,

	#[arg(long="tls-cert", help = "path to TLS Certificate for controller")]
	pub tls_cert: Option<String>,

	#[arg(long="tls-key", help = "path to TLS Key for controller")]
	pub tls_key: Option<String>,
}
