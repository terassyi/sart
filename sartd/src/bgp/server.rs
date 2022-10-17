use std::sync::{Arc, Mutex};
use tokio::net::{TcpListener, TcpStream};
use tokio_stream::{wrappers::TcpListenerStream, StreamExt};
use futures::{stream::{Stream, FuturesUnordered}, StreamExt};

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
            TcpListener::bind(format!("0.0.0.0:{}", server.config.lock().unwrap().port))
                .await
                .expect("cannot bind Ipv4 tcp listener")
        };
        let ipv6_listener = {
            TcpListener::bind(format!("[::]:{}", server.config.lock().unwrap().port))
                .await
                .expect("cannot bind ipv6 listener")
        };
        let mut s = TcpListenerStream::new(ipv4_listener).merge(TcpListenerStream::new(ipv6_listener));
        // let listeners = vec![ipv4_listener, ipv6_listener];
        // let mut listener_streams = listeners.into_iter().map(|l| TcpListenerStream::new(l))
        //         .collect::<Vec<TcpListenerStream>>();
        loop {
            futures::select_biased! {
                stream = s.next() => {
                    if let Some(stream) = stream {
                        let sock = stream.peer_addr().unwrap();
                        println!("{:?}", sock);
                    }
                }
            }
        }
    }
}

fn create_tcp_listener(addr: String, port: u16) -> io::Result<TcpListener> {

}

pub(crate) fn start(conf: Config) {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(Bgp::serve(conf));

    println!("sartd bgp server is now starting!");

    loop {}
}
