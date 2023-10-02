pub(crate) mod agent;
pub(crate) mod bgp;
pub(crate) mod controller;
pub(crate) mod fib;

use clap::{Parser, Subcommand, ValueEnum};

use crate::{
    bgp::{config::Config, server},
    trace::init::TraceConfig,
};

use self::{agent::AgentCmd, bgp::BgpCmd, fib::FibCmd, controller::ControllerCmd};

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
pub(crate) struct Cmd {
    #[arg(
        short,
        long,
        global = true,
        required = false,
        default_value = "info",
        help = "Log level(trace, debug, info, warn, error)"
    )]
    pub level: String,

    #[arg(
        value_enum,
        short = 'd',
        long,
        global = true,
        required = false,
        default_value = "plain",
        help = "Log display format"
    )]
    pub format: Format,

    #[arg(short = 'o', long = "log-file", help = "Log output file path")]
    pub log_file: Option<String>,

    #[clap(subcommand)]
    pub sub: SubCmd,
}

#[derive(Debug, Clone, Parser, ValueEnum)]
pub(crate) enum Format {
    Plain,
    Json,
}

impl ToString for Format {
    fn to_string(&self) -> String {
        match self {
            Format::Plain => "plain".to_string(),
            Format::Json => "json".to_string(),
        }
    }
}

#[derive(Debug, Clone, Subcommand)]
pub(crate) enum SubCmd {
    Bgp(BgpCmd),
    Fib(FibCmd),
    Agent(AgentCmd),
    Controller(ControllerCmd),
    Version,
}

pub(crate) fn main() {
    let command = Cmd::parse();

    let format = command.format;
    let level = command.level;
    let log_file = command.log_file;

    match command.sub {
        SubCmd::Version => println!("dev"),
        SubCmd::Bgp(b) => {
            let mut conf = match b.file {
                Some(file) => Config::load(&file).unwrap(),
                None => Config::default(),
            };
            if let Some(asn) = b.r#as {
                conf.asn = asn;
            }
            if let Some(router_id) = b.router_id {
                conf.router_id = router_id.parse().unwrap();
            }
            if let Some(fib_endpoint) = b.fib_endpoint {
                conf.fib_endpoint = Some(fib_endpoint);
            }
            if let Some(table_id) = b.fib_table_id {
                conf.fib_table = Some(table_id);
            }
            let trace_conf = TraceConfig {
                level,
                format: format.to_string(),
                file: log_file,
                _metrics_endpoint: None,
            };

            server::start(conf, trace_conf);
        }
        SubCmd::Fib(f) => {
            let trace_conf = TraceConfig {
                level,
                format: format.to_string(),
                file: log_file,
                _metrics_endpoint: None,
            };

            let mut config = match f.file {
                None => panic!("A configuration file is required for Fib manager"),
                Some(file) => crate::fib::config::Config::load(&file),
            }
            .unwrap();

            if !f.endpoint.is_empty() {
                config.endpoint = f.endpoint;
            }

            crate::fib::server::start(config, trace_conf);
        }
        SubCmd::Agent(a) => {
            let trace_conf = TraceConfig {
                level,
                format: format.to_string(),
                file: log_file,
                _metrics_endpoint: None,
            };
            let mut config = match a.file {
                None => crate::agent::config::Config::default(),
                Some(file) => crate::agent::config::Config::load(&file).unwrap(),
            };

            if !a.endpoint.is_empty() {
                config.endpoint = a.endpoint;
            }

            if let Some(cert) = a.tls_cert {
                config.tls.cert = cert;
            }
            if let Some(key) = a.tls_key {
                config.tls.key = key;
            }

            crate::agent::server::start(config, trace_conf);
        },
        SubCmd::Controller(c) => {
            let trace_conf = TraceConfig {
                level,
                format: format.to_string(),
                file: log_file,
                _metrics_endpoint: None,
            };
            let mut config = match c.file {
                None => crate::controller::config::Config::default(),
                Some(file) => crate::controller::config::Config::load(&file).unwrap(),
            };

            if !c.endpoint.is_empty() {
                config.endpoint = c.endpoint;
            }

            if let Some(cert) = c.tls_cert {
                config.tls.cert = cert;
            }
            if let Some(key) = c.tls_key {
                config.tls.key = key;
            }

            crate::controller::server::start(config, trace_conf);
        }
    }
}
