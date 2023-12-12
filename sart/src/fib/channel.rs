use clap::{Parser, Subcommand};

use crate::{
    cmd::Output,
    error::Error,
    proto::sart::{GetChannelRequest, ListChannelRequest},
    rpc::connect_fib,
};

use super::route::RouteCmd;

#[derive(Debug, Clone, Parser)]
pub(crate) struct ChannelCmd {
    #[structopt(subcommand)]
    pub action: Action,
}

#[derive(Debug, Clone, Subcommand)]
pub(crate) enum Action {
    Get { name: String },
    List,
    Route(RouteCmd),
}

pub(crate) async fn get(endpoint: &str, format: Output, name: String) -> Result<(), Error> {
    let mut client = connect_fib(endpoint).await;

    let res = client
        .get_channel(GetChannelRequest { name })
        .await
        .map_err(Error::FailedToGetResponse)?;

    match format {
        Output::Json => {}
        Output::Plain => {
            println!("Fib manager handling channel");
            if let Some(ch) = &res.get_ref().channel {
                println!("  Name: {}", ch.name);
                println!("  Subscribers");
                for s in ch.subscribers.iter() {
                    println!("    Type: {}", s.r#type);
                    match s.r#type.as_str() {
                        "bgp" => {
                            println!("    Endpoint: {}", s.endpoint);
                        }
                        "kernel" => {
                            println!("    Tables: {:?}", s.tables);
                        }
                        _ => return Err(Error::InvalidOriginValue),
                    }
                }
                println!("  Publishers");
                for p in ch.publishers.iter() {
                    println!("    Type: {}", p.r#type);
                    match p.r#type.as_str() {
                        "bgp" => {
                            println!("    Endpoint: {}", p.endpoint);
                        }
                        "kernel" => {
                            println!("    Tables: {:?}", p.tables);
                        }
                        _ => return Err(Error::InvalidChannelType),
                    }
                }
            }
        }
    }
    Ok(())
}

pub(crate) async fn list(endpoint: &str, format: Output) -> Result<(), Error> {
    let mut client = connect_fib(endpoint).await;

    let res = client
        .list_channel(ListChannelRequest {})
        .await
        .map_err(Error::FailedToGetResponse)?;

    match format {
        Output::Json => {}
        Output::Plain => {
            println!("FIB manager handling channels");
            for ch in res.get_ref().channels.iter() {
                println!("Name: {}", ch.name);
                println!("  Subscribers");
                for s in ch.subscribers.iter() {
                    println!("    Type: {}", s.r#type);
                    match s.r#type.as_str() {
                        "bgp" => {
                            println!("    Endpoint: {}", s.endpoint);
                        }
                        "kernel" => {
                            println!("    Tables: {:?}", s.tables);
                        }
                        _ => return Err(Error::InvalidOriginValue),
                    }
                }
                println!("  Publishers");
                for p in ch.publishers.iter() {
                    println!("    Type: {}", p.r#type);
                    match p.r#type.as_str() {
                        "bgp" => {
                            println!("    Endpoint: {}", p.endpoint);
                        }
                        "kernel" => {
                            println!("    Tables: {:?}", p.tables);
                        }
                        _ => return Err(Error::InvalidChannelType),
                    }
                }
                println!("---")
            }
        }
    }
    Ok(())
}
