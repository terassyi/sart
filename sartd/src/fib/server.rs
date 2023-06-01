use futures::FutureExt;
use tokio_stream::StreamExt;
use tonic::{Request, Response, Status};

use crate::{
    fib::{kernel::KernelRtPoller, rib::RequestType, rtnetlink::iniit_rtnetlink_handler, channel::Protocol},
    proto::sart::{
        fib_manager_api_server::{FibManagerApi, FibManagerApiServer},
        GetChannelResponse, ListChannelResponse,
    },
    proto::sart::{GetChannelRequest, GetRoutesRequest, GetRoutesResponse, ListChannelRequest},
    trace::{prepare_tracing, TraceConfig},
};

use super::{channel::Channel, config::Config};

pub mod api {
    pub(crate) const FILE_DESCRIPTOR_SET: &[u8] = tonic::include_file_descriptor_set!("sartd");
}

pub(crate) fn start(config: Config, trace: TraceConfig) {
    let server = Fib::new(config);
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(run(server, trace));
}

#[derive(Debug)]
pub(crate) struct Fib {
    endpoint: String,
    channels: Vec<Channel>,
}

impl Fib {
    pub fn new(config: Config) -> Fib {
        Fib {
            endpoint: config.endpoint,
            channels: config.channels,
        }
    }
}

#[tonic::async_trait]
impl FibManagerApi for Fib {
    async fn get_channel(
        &self,
        req: Request<GetChannelRequest>,
    ) -> Result<Response<GetChannelResponse>, Status> {
        let name = &req.get_ref().name;
        if let Some(channel) = self.channels.iter().find(|&ch| ch.name.eq(name)) {
            let res = crate::proto::sart::Channel::from(channel);
            Ok(Response::new(GetChannelResponse{
                channel: Some(res)
            }))
        } else {
            Err(Status::not_found(format!("{} is not found", name)))
        }
    }

    async fn list_channel(
        &self,
        _req: Request<ListChannelRequest>,
    ) -> Result<Response<ListChannelResponse>, Status> {
        let channels: Vec<crate::proto::sart::Channel> = self.channels.iter().map(crate::proto::sart::Channel::from).collect();
        Ok(Response::new(ListChannelResponse{
            channels
        }))
    }

    async fn get_routes(
        &self,
        req: Request<GetRoutesRequest>,
    ) -> Result<Response<GetRoutesResponse>, Status> {
        let name = &req.get_ref().channel;
        if let Some(channel) = self.channels.iter().find(|&ch| ch.name.eq(name)) {
            match channel.list_routes() {
                Some(routes) => {
                    let routes = routes.iter().map(crate::proto::sart::Route::from).collect();
                    Ok(Response::new(GetRoutesResponse{
                        routes
                    }))
                }
                None => Ok(Response::new(GetRoutesResponse { routes: Vec::new() }))
            }
        } else {
            Err(Status::not_found(format!("{} channel is not found", name)))
        }
    }
}

async fn run(server: Fib, trace_config: TraceConfig) {
    prepare_tracing(trace_config);

    let mut poller = KernelRtPoller::new();

    let channels = server.channels.clone();
    
    let handler = iniit_rtnetlink_handler().await.unwrap();

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
                        if let Some((req, route)) = request {
                            let res = match req {
                                RequestType::AddRoute => {
                                    ch.register_route(route)
                                },
                                RequestType::AddMultiPathRoute => {
                                    match ch.get_route(&route.destination, route.protocol) {
                                        Some(existing_route) => {
                                            match existing_route.merge_multipath(route) {
                                                Ok(r) => {
                                                    ch.register_route(r)
                                                },
                                                Err(e) => {
                                                    tracing::error!(error=?e, "failed to append multipath entry from route");
                                                    continue;
                                                }
                                            }

                                        }
                                        None => {
                                            ch.register_route(route)
                                        }
                                    }

                                },
                                RequestType::DeleteRoute => {
                                    ch.remove_route(route)
                                },
                                RequestType::DeleteMultiPathRoute => {
                                    match ch.get_route(&route.destination, route.protocol) {
                                        Some(existing_route) => {
                                            match existing_route.pop_multipath(route) {
                                                Ok(r) => {
                                                    ch.register_route(r)
                                                },
                                                Err(e) => {
                                                    tracing::error!(error=?e, "failed to pop multipath entry from route");
                                                    continue;
                                                }
                                            }
                                        },
                                        None => {
                                            ch.remove_route(route)
                                        }
                                    }
                                }
                            };
                            match res {
                                Some(route) => {
                                    for p in ch.publishers.iter() {
                                        let res = match p {
                                            Protocol::Bgp(b) => b.publish(req, route.clone()).await,
                                            Protocol::Kernel(k) => {
                                                let handler = handler.clone().lock().unwrap();
                                                k.publish(req, route.clone(), handler).await
                                            }
                                        };
                                        match res {
                                            Ok(_) => {},
                                            Err(e) => tracing::error!(error=?e, route=?route,"failed to publish the route"),
                                        }
                                    }
                                },
                                None => {
                                    tracing::info!("published route is not found.");
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

    tracing::info!("API server start to listen at {}", server.endpoint);
    let endpoint = server.endpoint.clone();
    let sock_addr = endpoint.parse().unwrap();

    let reflection = tonic_reflection::server::Builder::configure()
        .register_encoded_file_descriptor_set(api::FILE_DESCRIPTOR_SET)
        .build()
        .unwrap();

    tracing::info!("start to listen at {}", endpoint);
    tonic::transport::Server::builder()
        .add_service(FibManagerApiServer::new(server))
        .add_service(reflection)
        .serve(sock_addr)
        .await
        .unwrap();
}
