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
}
