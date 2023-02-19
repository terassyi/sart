pub(crate) mod bgp;
pub(crate) mod proto;
pub(crate) mod util;

use bgp::config::TraceConfig;
use std::{net::Ipv4Addr, str::FromStr};

use crate::bgp::config::Config;
use crate::bgp::server;
use clap::{App, Arg};

fn main() -> Result<(), std::io::Error> {
    let app = App::new("sartd-bgp")
        .version("v0.0.1")
        .arg(
            Arg::with_name("config")
                .short('f')
                .long("config-file")
                .takes_value(true)
                .required(false)
                .help("config file for a sartd-bgp daemon"),
        )
        .arg(
            Arg::with_name("version")
                .short('v')
                .long("version")
                .takes_value(false)
                .required(false)
                .help("version of sartd-bgp"),
        )
        .arg(
            Arg::with_name("as_number")
                .short('a')
                .long("as")
                .takes_value(true)
                .required(false)
                .help("my AS number"),
        )
        .arg(
            Arg::with_name("router_id")
                .short('r')
                .long("router-id")
                .takes_value(true)
                .required(false)
                .help("router-id(IPv4 address format"),
        )
        .arg(
            Arg::with_name("log_level")
                .short('l')
                .long("log-level")
                .takes_value(true)
                .required(false)
                .default_value("info")
                .help("log-level(error,warn,info,debug,trace)"),
        )
        .arg(
            Arg::with_name("format")
                .long("format")
                .takes_value(true)
                .required(false)
                .default_value("plain")
                .help("log fotmat(plain,json"),
        )
        .arg(
            Arg::with_name("log_file")
                .short('o')
                .long("log-file")
                .takes_value(true)
                .required(false)
                .default_value("")
                .help("log output file"),
        )
        .get_matches();
    let conf = if let Some(file) = app.value_of("config") {
        let mut conf = Config::load(file).expect("failed to load config");
        if let Some(asn) = app.value_of("as_number") {
            conf.asn = asn.parse::<u32>().unwrap();
        }
        if let Some(router_id) = app.value_of("router_id") {
            conf.router_id = Ipv4Addr::from_str(router_id).expect("invalid router-id");
        }
        conf
    } else {
        let mut conf = Config::default();
        if let Some(asn) = app.value_of("as_number") {
            conf.set_as_number(asn.parse::<u32>().unwrap())
        }
        if let Some(router_id) = app.value_of("router_id") {
            conf.set_router_id(Ipv4Addr::from_str(router_id).unwrap())
        }
        conf
    };

    let level = app.value_of("log_level").unwrap();
    let format = app.value_of("format").unwrap();
    let log_file = app.value_of("log_file").unwrap();
    let log_file = if log_file.is_empty() {
        None
    } else {
        Some(log_file.to_string())
    };

    let trace_config = TraceConfig {
        level: level.to_string(),
        format: format.to_string(),
        file: log_file,
        metrics_endpoint: None,
    };

    server::start(conf, trace_config);
    Ok(())
}
