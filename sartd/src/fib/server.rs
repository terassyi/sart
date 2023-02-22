use crate::{
    fib::api_server::api,
    proto::sart::fib_api_server::FibApiServer,
    trace::{prepare_tracing, TraceConfig},
};

use super::api_server::FibServer;

pub(crate) fn start(endpoint: String, trace: TraceConfig) {
    let server = Fib::new(endpoint);
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(server.run(trace));
}

#[derive(Debug)]
pub(crate) struct Fib {
    endpoint: String,
}

impl Fib {
    pub fn new(endpoint: String) -> Fib {
        Fib { endpoint }
    }

    pub async fn run(&self, trace_config: TraceConfig) {
        prepare_tracing(trace_config);

        // rt_netlink
        let (conn, handler, _rx) = rtnetlink::new_connection().unwrap();
        tokio::spawn(conn);

        // run gRPC fi server
        let endpoint = self.endpoint.clone();
        let sock_addr = endpoint.parse().unwrap();

        let reflection = tonic_reflection::server::Builder::configure()
            .register_encoded_file_descriptor_set(api::FILE_DESCRIPTOR_SET)
            .build()
            .unwrap();

        tonic::transport::Server::builder()
            .add_service(FibApiServer::new(FibServer::new(handler)))
            .add_service(reflection)
            .serve(sock_addr)
            .await
            .unwrap();
    }
}
