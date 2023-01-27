pub(crate) mod bgp;
pub(crate) mod proto;

use std::{net::Ipv4Addr, str::FromStr};
use tracing::{Level};
use tracing_subscriber::{self};

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
        .get_matches();
    let conf = if let Some(file) = app.value_of("config") {
        let mut conf = Config::load(file).expect("failed to load config");
        match app.value_of("as_number") {
            Some(asn) => {
                conf.asn = asn.parse::<u32>().unwrap();
            }
            None => {}
        }
        match app.value_of("router_id") {
            Some(router_id) => {
                conf.router_id = Ipv4Addr::from_str(router_id).expect("invalid router-id");
            }
            None => {}
        }
        conf
    } else {
        let mut conf = Config::default();
        match app.value_of("as_number") {
            Some(asn) => conf.set_as_number(asn.parse::<u32>().unwrap()),
            None => {}
        }
        match app.value_of("router_id") {
            Some(router_id) => conf.set_router_id(Ipv4Addr::from_str(router_id).unwrap()),
            None => {}
        }
        conf
    };

    let level = app.value_of("log_level").unwrap();
    let format = app.value_of("format").unwrap();
    prepare_tracing(level, format);

    server::start(conf);
    Ok(())
}

fn prepare_tracing(level: &str, format: &str) {
    if format == "json" {
        tracing_subscriber::fmt()
            .with_max_level(Level::from_str(level).unwrap())
            .json()
            .init();
    } else {
        tracing_subscriber::fmt()
            .with_max_level(Level::from_str(level).unwrap())
            .init();
    }
}
