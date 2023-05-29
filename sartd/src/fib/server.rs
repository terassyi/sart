use futures::FutureExt;
use tokio_stream::StreamExt;

use crate::{
    fib::{api_server::api, kernel::KernelRtPoller, rib::RequestType},
    proto::sart::fib_api_server::FibApiServer,
    trace::{prepare_tracing, TraceConfig},
};

use super::{api_server::FibServer, config::Config};

pub(crate) fn start(endpoint: String, config: Config, trace: TraceConfig) {
    let server = Fib::new(endpoint, config);
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(server.run(trace));
}

#[derive(Debug)]
pub(crate) struct Fib {
    endpoint: String,
    config: Config,
}

impl Fib {
    pub fn new(endpoint: String, config: Config) -> Fib {
        Fib { endpoint, config }
    }

    #[tracing::instrument(skip(self, trace_config))]
    pub async fn run(&self, trace_config: TraceConfig) {
        prepare_tracing(trace_config);

        let mut poller = KernelRtPoller::new();

        let channels = self.config.channels.clone();

        for mut ch in channels.into_iter() {
            let receivers = ch.register(&mut poller).await.unwrap();

            let mut fused_receivers = futures::stream::select_all(
                receivers
                    .into_iter()
                    .map(tokio_stream::wrappers::ReceiverStream::new),
            );

            tracing::info!(channel=?ch, "start to subscribe");
            tokio::spawn(async move {
                loop {
                    futures::select_biased! {
                        request = fused_receivers.next().fuse() => {
                            if let Some(request) = request {
                                let res = match request.0 {
                                    RequestType::AddRoute | RequestType::AddMultiPathRoute => {
                                        ch.register_route(request.1)
                                    },
                                    RequestType::DeleteRoute | RequestType::DeleteMultiPathRoute => {
                                        ch.remove_route(request.1)
                                    },
                                };
                                match res {
                                    Ok(route) => {
                                        for p in ch.publishers.iter() {
                                            match p.publish(route.clone()).await {
                                                Ok(_) => {},
                                                Err(e) => tracing::error!(error=?e, "failed to publish the route")
                                            }
                                        }
                                    },
                                    Err(e) => {
                                        tracing::error!(error=?e, "failed to handle rib");
                                    }
                                }
                            }
                        }
                    }
                }
            });
        }
        // run poller
        tokio::spawn(async move {
            poller.run().await.unwrap();
        });
        

        // rt_netlink
        let (conn, handler, _rx) = rtnetlink::new_connection().unwrap();
        tokio::spawn(conn);

        // run gRPC fi server

        tracing::info!("API server start to listen at {}", self.endpoint);
        let endpoint = self.endpoint.clone();
        let sock_addr = endpoint.parse().unwrap();

        let reflection = tonic_reflection::server::Builder::configure()
            .register_encoded_file_descriptor_set(api::FILE_DESCRIPTOR_SET)
            .build()
            .unwrap();

        tracing::info!("start to listen at {}", endpoint);
        tonic::transport::Server::builder()
            .add_service(FibApiServer::new(FibServer::new(handler)))
            .add_service(reflection)
            .serve(sock_addr)
            .await
            .unwrap();
    }
}
