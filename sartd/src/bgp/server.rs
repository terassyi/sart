use std::sync::{Arc, Mutex};
use tokio_stream::{self, wrappers::TcpListenerStream, StreamExt};

use crate::bgp::config::Config;
#[derive(Debug)]
pub(crate) struct Bgp {
    config: Arc<Mutex<Config>>,
}

impl Bgp {
    pub const BGP_PORT: u16 = 179;
    pub fn new(conf: Config) -> Self {
        Self {
            config: Arc::new(Mutex::new(conf)),
        }
    }

    pub async fn serve(conf: Config) {
        let server = Self::new(conf);

        let ipv4_listener = {
            tokio::net::TcpListener::bind(format!(":{}", server.config.lock().unwrap().port))
                .await
                .expect("cannot bind Ipv4 tcp listener")
        };
        let ipv6_listener = {
            tokio::net::TcpListener::bind(format!("[::]:{}", server.config.lock().unwrap().port))
                .await
                .expect("cannot bind ipv6 listener")
        };
        let mut listeners = tokio_stream::wrappers::TcpListenerStream::new(ipv4_listener).merge(
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
