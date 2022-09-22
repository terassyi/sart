use std::fmt::format;
use tokio_stream::{self, wrappers::TcpListenerStream, StreamExt};

use crate::bgp::config::Config;
#[derive(Debug)]
pub(crate) struct Bgp {
    config: Config,
}

impl Bgp {
    pub const BGP_PORT: u16 = 179;
    pub fn new(conf: Config) -> Self {
        Self { config: conf }
    }

    pub async fn serve(conf: Config) {
        let server = Self::new(conf);

        let ipv4_listener = tokio::net::TcpListener::bind(format!(":{}", server.config.port))
            .await
            .expect("cannot bind Ipv4 tcp listener");
        let ipv6_listener = tokio::net::TcpListener::bind(format!("[::]:{}", server.config.port))
            .await
            .expect("cannot bind ipv6 listener");
        let mut listener = tokio_stream::wrappers::TcpListenerStream::new(ipv4_listener).merge(
            tokio_stream::wrappers::TcpListenerStream::new(ipv6_listener),
        );
    }
}

pub(crate) fn start(conf: Config) {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(Bgp::serve(conf));

    println!("sartd bgp server is now starting!");
}
