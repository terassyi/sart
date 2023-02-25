#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct HealthRequest {}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetBgpInfoRequest {}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetBgpInfoResponse {
    #[prost(message, optional, tag = "1")]
    pub info: ::core::option::Option<BgpInfo>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetNeighborRequest {
    #[prost(string, tag = "1")]
    pub addr: ::prost::alloc::string::String,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetNeighborResponse {
    #[prost(message, optional, tag = "1")]
    pub peer: ::core::option::Option<Peer>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetPathRequest {
    #[prost(message, optional, tag = "1")]
    pub family: ::core::option::Option<AddressFamily>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetPathResponse {
    #[prost(message, repeated, tag = "1")]
    pub paths: ::prost::alloc::vec::Vec<Path>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct SetAsRequest {
    #[prost(uint32, tag = "1")]
    pub asn: u32,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct SetRouterIdRequest {
    #[prost(string, tag = "1")]
    pub router_id: ::prost::alloc::string::String,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct AddPeerRequest {
    #[prost(message, optional, tag = "1")]
    pub peer: ::core::option::Option<Peer>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct DeletePeerRequest {
    #[prost(string, tag = "1")]
    pub addr: ::prost::alloc::string::String,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct AddPathRequest {
    #[prost(message, optional, tag = "1")]
    pub family: ::core::option::Option<AddressFamily>,
    #[prost(string, repeated, tag = "2")]
    pub prefixes: ::prost::alloc::vec::Vec<::prost::alloc::string::String>,
    #[prost(message, repeated, tag = "3")]
    pub attributes: ::prost::alloc::vec::Vec<::prost_types::Any>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct AddPathResponse {}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct DeletePathRequest {
    #[prost(message, optional, tag = "1")]
    pub family: ::core::option::Option<AddressFamily>,
    #[prost(string, repeated, tag = "2")]
    pub prefixes: ::prost::alloc::vec::Vec<::prost::alloc::string::String>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct DeletePathResponse {}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct BgpInfo {
    #[prost(uint32, tag = "1")]
    pub asn: u32,
    #[prost(string, tag = "2")]
    pub router_id: ::prost::alloc::string::String,
    #[prost(uint32, tag = "3")]
    pub port: u32,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct AddressFamily {
    #[prost(enumeration = "address_family::Afi", tag = "1")]
    pub afi: i32,
    #[prost(enumeration = "address_family::Safi", tag = "2")]
    pub safi: i32,
}
/// Nested message and enum types in `AddressFamily`.
pub mod address_family {
    #[derive(
        Clone,
        Copy,
        Debug,
        PartialEq,
        Eq,
        Hash,
        PartialOrd,
        Ord,
        ::prost::Enumeration
    )]
    #[repr(i32)]
    pub enum Afi {
        Unknown = 0,
        Ip4 = 1,
        Ip6 = 2,
    }
    impl Afi {
        /// String value of the enum field names used in the ProtoBuf definition.
        ///
        /// The values are not transformed in any way and thus are considered stable
        /// (if the ProtoBuf definition does not change) and safe for programmatic use.
        pub fn as_str_name(&self) -> &'static str {
            match self {
                Afi::Unknown => "AFI_UNKNOWN",
                Afi::Ip4 => "AFI_IP4",
                Afi::Ip6 => "AFI_IP6",
            }
        }
    }
    #[derive(
        Clone,
        Copy,
        Debug,
        PartialEq,
        Eq,
        Hash,
        PartialOrd,
        Ord,
        ::prost::Enumeration
    )]
    #[repr(i32)]
    pub enum Safi {
        Unknown = 0,
        Unicast = 1,
        Multicast = 2,
    }
    impl Safi {
        /// String value of the enum field names used in the ProtoBuf definition.
        ///
        /// The values are not transformed in any way and thus are considered stable
        /// (if the ProtoBuf definition does not change) and safe for programmatic use.
        pub fn as_str_name(&self) -> &'static str {
            match self {
                Safi::Unknown => "SAFI_UNKNOWN",
                Safi::Unicast => "SAFI_UNICAST",
                Safi::Multicast => "SAFI_MULTICAST",
            }
        }
    }
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Peer {
    #[prost(uint32, tag = "1")]
    pub asn: u32,
    #[prost(string, tag = "2")]
    pub address: ::prost::alloc::string::String,
    #[prost(string, tag = "3")]
    pub router_id: ::prost::alloc::string::String,
    #[prost(message, repeated, tag = "4")]
    pub families: ::prost::alloc::vec::Vec<AddressFamily>,
    #[prost(uint32, tag = "5")]
    pub hold_time: u32,
    #[prost(uint32, tag = "6")]
    pub keepalive_time: u32,
    #[prost(message, optional, tag = "7")]
    pub uptime: ::core::option::Option<::prost_types::Timestamp>,
    #[prost(message, optional, tag = "8")]
    pub send_counter: ::core::option::Option<MessageCounter>,
    #[prost(message, optional, tag = "9")]
    pub recv_counter: ::core::option::Option<MessageCounter>,
    #[prost(enumeration = "peer::State", tag = "10")]
    pub state: i32,
    #[prost(bool, tag = "11")]
    pub passive_open: bool,
}
/// Nested message and enum types in `Peer`.
pub mod peer {
    #[derive(
        Clone,
        Copy,
        Debug,
        PartialEq,
        Eq,
        Hash,
        PartialOrd,
        Ord,
        ::prost::Enumeration
    )]
    #[repr(i32)]
    pub enum State {
        Unknown = 0,
        Idle = 1,
        Connect = 2,
        Active = 3,
        OpenSent = 4,
        OpenConfirm = 5,
        Established = 6,
    }
    impl State {
        /// String value of the enum field names used in the ProtoBuf definition.
        ///
        /// The values are not transformed in any way and thus are considered stable
        /// (if the ProtoBuf definition does not change) and safe for programmatic use.
        pub fn as_str_name(&self) -> &'static str {
            match self {
                State::Unknown => "UNKNOWN",
                State::Idle => "IDLE",
                State::Connect => "CONNECT",
                State::Active => "ACTIVE",
                State::OpenSent => "OPEN_SENT",
                State::OpenConfirm => "OPEN_CONFIRM",
                State::Established => "ESTABLISHED",
            }
        }
    }
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Path {
    #[prost(string, tag = "1")]
    pub nlri: ::prost::alloc::string::String,
    #[prost(message, optional, tag = "2")]
    pub family: ::core::option::Option<AddressFamily>,
    #[prost(uint32, tag = "3")]
    pub origin: u32,
    #[prost(string, repeated, tag = "4")]
    pub next_hops: ::prost::alloc::vec::Vec<::prost::alloc::string::String>,
    #[prost(message, repeated, tag = "5")]
    pub segments: ::prost::alloc::vec::Vec<AsSegment>,
    #[prost(uint32, tag = "6")]
    pub local_pref: u32,
    #[prost(uint32, tag = "7")]
    pub med: u32,
    #[prost(uint32, tag = "8")]
    pub peer_asn: u32,
    #[prost(string, tag = "9")]
    pub peer_addr: ::prost::alloc::string::String,
    #[prost(bool, tag = "10")]
    pub best: bool,
    #[prost(message, optional, tag = "11")]
    pub timestamp: ::core::option::Option<::prost_types::Timestamp>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct OriginAttribute {
    #[prost(uint32, tag = "1")]
    pub value: u32,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct AsSegment {
    #[prost(enumeration = "as_segment::Type", tag = "1")]
    pub r#type: i32,
    #[prost(uint32, repeated, tag = "2")]
    pub elm: ::prost::alloc::vec::Vec<u32>,
}
/// Nested message and enum types in `AsSegment`.
pub mod as_segment {
    #[derive(
        Clone,
        Copy,
        Debug,
        PartialEq,
        Eq,
        Hash,
        PartialOrd,
        Ord,
        ::prost::Enumeration
    )]
    #[repr(i32)]
    pub enum Type {
        Unknown = 0,
        AsSet = 1,
        AsSequence = 2,
    }
    impl Type {
        /// String value of the enum field names used in the ProtoBuf definition.
        ///
        /// The values are not transformed in any way and thus are considered stable
        /// (if the ProtoBuf definition does not change) and safe for programmatic use.
        pub fn as_str_name(&self) -> &'static str {
            match self {
                Type::Unknown => "UNKNOWN",
                Type::AsSet => "AS_SET",
                Type::AsSequence => "AS_SEQUENCE",
            }
        }
    }
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct AsPathAttribute {
    #[prost(message, repeated, tag = "1")]
    pub segments: ::prost::alloc::vec::Vec<AsSegment>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct NextHopAttribute {
    #[prost(string, tag = "1")]
    pub value: ::prost::alloc::string::String,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct MultiExitDiscAttribute {
    #[prost(uint32, tag = "1")]
    pub value: u32,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct LocalPrefAttribute {
    #[prost(uint32, tag = "1")]
    pub value: u32,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct AtomicAggregateAttribute {}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct AggregatorAttribute {
    #[prost(uint32, tag = "1")]
    pub asn: u32,
    #[prost(string, tag = "2")]
    pub address: ::prost::alloc::string::String,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct MessageCounter {
    #[prost(uint32, tag = "1")]
    pub open: u32,
    #[prost(uint32, tag = "2")]
    pub update: u32,
    #[prost(uint32, tag = "3")]
    pub keepalive: u32,
    #[prost(uint32, tag = "4")]
    pub notification: u32,
    #[prost(uint32, tag = "5")]
    pub route_refresh: u32,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct UnknownAttribute {
    #[prost(uint32, tag = "1")]
    pub flags: u32,
    #[prost(uint32, tag = "2")]
    pub code: u32,
    #[prost(bytes = "vec", tag = "3")]
    pub data: ::prost::alloc::vec::Vec<u8>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct MultiProtocolCapability {
    #[prost(message, optional, tag = "1")]
    pub family: ::core::option::Option<AddressFamily>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct RouteRefreshCapability {}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GracefulRestartCapabilityTuple {
    #[prost(message, optional, tag = "1")]
    pub family: ::core::option::Option<AddressFamily>,
    #[prost(uint32, tag = "2")]
    pub flags: u32,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GracefulRestartCapability {
    #[prost(uint32, tag = "1")]
    pub flags: u32,
    #[prost(uint32, tag = "2")]
    pub time: u32,
    #[prost(message, repeated, tag = "3")]
    pub tuples: ::prost::alloc::vec::Vec<GracefulRestartCapabilityTuple>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct FourOctedAsnCapability {
    #[prost(uint32, tag = "1")]
    pub asn: u32,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct EnhancedRouteRefreshCapability {}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct UnknownCapability {
    #[prost(uint32, tag = "1")]
    pub code: u32,
    #[prost(bytes = "vec", tag = "2")]
    pub value: ::prost::alloc::vec::Vec<u8>,
}
/// Generated client implementations.
pub mod bgp_api_client {
    #![allow(unused_variables, dead_code, missing_docs, clippy::let_unit_value)]
    use tonic::codegen::*;
    use tonic::codegen::http::Uri;
    #[derive(Debug, Clone)]
    pub struct BgpApiClient<T> {
        inner: tonic::client::Grpc<T>,
    }
    impl BgpApiClient<tonic::transport::Channel> {
        /// Attempt to create a new client by connecting to a given endpoint.
        pub async fn connect<D>(dst: D) -> Result<Self, tonic::transport::Error>
        where
            D: std::convert::TryInto<tonic::transport::Endpoint>,
            D::Error: Into<StdError>,
        {
            let conn = tonic::transport::Endpoint::new(dst)?.connect().await?;
            Ok(Self::new(conn))
        }
    }
    impl<T> BgpApiClient<T>
    where
        T: tonic::client::GrpcService<tonic::body::BoxBody>,
        T::Error: Into<StdError>,
        T::ResponseBody: Body<Data = Bytes> + Send + 'static,
        <T::ResponseBody as Body>::Error: Into<StdError> + Send,
    {
        pub fn new(inner: T) -> Self {
            let inner = tonic::client::Grpc::new(inner);
            Self { inner }
        }
        pub fn with_origin(inner: T, origin: Uri) -> Self {
            let inner = tonic::client::Grpc::with_origin(inner, origin);
            Self { inner }
        }
        pub fn with_interceptor<F>(
            inner: T,
            interceptor: F,
        ) -> BgpApiClient<InterceptedService<T, F>>
        where
            F: tonic::service::Interceptor,
            T::ResponseBody: Default,
            T: tonic::codegen::Service<
                http::Request<tonic::body::BoxBody>,
                Response = http::Response<
                    <T as tonic::client::GrpcService<tonic::body::BoxBody>>::ResponseBody,
                >,
            >,
            <T as tonic::codegen::Service<
                http::Request<tonic::body::BoxBody>,
            >>::Error: Into<StdError> + Send + Sync,
        {
            BgpApiClient::new(InterceptedService::new(inner, interceptor))
        }
        /// Compress requests with the given encoding.
        ///
        /// This requires the server to support it otherwise it might respond with an
        /// error.
        #[must_use]
        pub fn send_compressed(mut self, encoding: CompressionEncoding) -> Self {
            self.inner = self.inner.send_compressed(encoding);
            self
        }
        /// Enable decompressing responses.
        #[must_use]
        pub fn accept_compressed(mut self, encoding: CompressionEncoding) -> Self {
            self.inner = self.inner.accept_compressed(encoding);
            self
        }
        pub async fn health(
            &mut self,
            request: impl tonic::IntoRequest<super::HealthRequest>,
        ) -> Result<tonic::Response<()>, tonic::Status> {
            self.inner
                .ready()
                .await
                .map_err(|e| {
                    tonic::Status::new(
                        tonic::Code::Unknown,
                        format!("Service was not ready: {}", e.into()),
                    )
                })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static("/sart.BgpApi/Health");
            self.inner.unary(request.into_request(), path, codec).await
        }
        pub async fn get_bgp_info(
            &mut self,
            request: impl tonic::IntoRequest<super::GetBgpInfoRequest>,
        ) -> Result<tonic::Response<super::GetBgpInfoResponse>, tonic::Status> {
            self.inner
                .ready()
                .await
                .map_err(|e| {
                    tonic::Status::new(
                        tonic::Code::Unknown,
                        format!("Service was not ready: {}", e.into()),
                    )
                })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static("/sart.BgpApi/GetBgpInfo");
            self.inner.unary(request.into_request(), path, codec).await
        }
        pub async fn get_neighbor(
            &mut self,
            request: impl tonic::IntoRequest<super::GetNeighborRequest>,
        ) -> Result<tonic::Response<super::GetNeighborResponse>, tonic::Status> {
            self.inner
                .ready()
                .await
                .map_err(|e| {
                    tonic::Status::new(
                        tonic::Code::Unknown,
                        format!("Service was not ready: {}", e.into()),
                    )
                })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static("/sart.BgpApi/GetNeighbor");
            self.inner.unary(request.into_request(), path, codec).await
        }
        /// rpc ListNeighbor(ListNeighborRequest) returns (ListNeighborResponse);
        pub async fn get_path(
            &mut self,
            request: impl tonic::IntoRequest<super::GetPathRequest>,
        ) -> Result<tonic::Response<super::GetPathResponse>, tonic::Status> {
            self.inner
                .ready()
                .await
                .map_err(|e| {
                    tonic::Status::new(
                        tonic::Code::Unknown,
                        format!("Service was not ready: {}", e.into()),
                    )
                })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static("/sart.BgpApi/GetPath");
            self.inner.unary(request.into_request(), path, codec).await
        }
        pub async fn set_as(
            &mut self,
            request: impl tonic::IntoRequest<super::SetAsRequest>,
        ) -> Result<tonic::Response<()>, tonic::Status> {
            self.inner
                .ready()
                .await
                .map_err(|e| {
                    tonic::Status::new(
                        tonic::Code::Unknown,
                        format!("Service was not ready: {}", e.into()),
                    )
                })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static("/sart.BgpApi/SetAS");
            self.inner.unary(request.into_request(), path, codec).await
        }
        pub async fn set_router_id(
            &mut self,
            request: impl tonic::IntoRequest<super::SetRouterIdRequest>,
        ) -> Result<tonic::Response<()>, tonic::Status> {
            self.inner
                .ready()
                .await
                .map_err(|e| {
                    tonic::Status::new(
                        tonic::Code::Unknown,
                        format!("Service was not ready: {}", e.into()),
                    )
                })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static("/sart.BgpApi/SetRouterId");
            self.inner.unary(request.into_request(), path, codec).await
        }
        pub async fn add_peer(
            &mut self,
            request: impl tonic::IntoRequest<super::AddPeerRequest>,
        ) -> Result<tonic::Response<()>, tonic::Status> {
            self.inner
                .ready()
                .await
                .map_err(|e| {
                    tonic::Status::new(
                        tonic::Code::Unknown,
                        format!("Service was not ready: {}", e.into()),
                    )
                })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static("/sart.BgpApi/AddPeer");
            self.inner.unary(request.into_request(), path, codec).await
        }
        pub async fn delete_peer(
            &mut self,
            request: impl tonic::IntoRequest<super::DeletePeerRequest>,
        ) -> Result<tonic::Response<()>, tonic::Status> {
            self.inner
                .ready()
                .await
                .map_err(|e| {
                    tonic::Status::new(
                        tonic::Code::Unknown,
                        format!("Service was not ready: {}", e.into()),
                    )
                })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static("/sart.BgpApi/DeletePeer");
            self.inner.unary(request.into_request(), path, codec).await
        }
        pub async fn add_path(
            &mut self,
            request: impl tonic::IntoRequest<super::AddPathRequest>,
        ) -> Result<tonic::Response<super::AddPathResponse>, tonic::Status> {
            self.inner
                .ready()
                .await
                .map_err(|e| {
                    tonic::Status::new(
                        tonic::Code::Unknown,
                        format!("Service was not ready: {}", e.into()),
                    )
                })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static("/sart.BgpApi/AddPath");
            self.inner.unary(request.into_request(), path, codec).await
        }
        pub async fn delete_path(
            &mut self,
            request: impl tonic::IntoRequest<super::DeletePathRequest>,
        ) -> Result<tonic::Response<super::DeletePathResponse>, tonic::Status> {
            self.inner
                .ready()
                .await
                .map_err(|e| {
                    tonic::Status::new(
                        tonic::Code::Unknown,
                        format!("Service was not ready: {}", e.into()),
                    )
                })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static("/sart.BgpApi/DeletePath");
            self.inner.unary(request.into_request(), path, codec).await
        }
    }
}
/// Generated server implementations.
pub mod bgp_api_server {
    #![allow(unused_variables, dead_code, missing_docs, clippy::let_unit_value)]
    use tonic::codegen::*;
    /// Generated trait containing gRPC methods that should be implemented for use with BgpApiServer.
    #[async_trait]
    pub trait BgpApi: Send + Sync + 'static {
        async fn health(
            &self,
            request: tonic::Request<super::HealthRequest>,
        ) -> Result<tonic::Response<()>, tonic::Status>;
        async fn get_bgp_info(
            &self,
            request: tonic::Request<super::GetBgpInfoRequest>,
        ) -> Result<tonic::Response<super::GetBgpInfoResponse>, tonic::Status>;
        async fn get_neighbor(
            &self,
            request: tonic::Request<super::GetNeighborRequest>,
        ) -> Result<tonic::Response<super::GetNeighborResponse>, tonic::Status>;
        /// rpc ListNeighbor(ListNeighborRequest) returns (ListNeighborResponse);
        async fn get_path(
            &self,
            request: tonic::Request<super::GetPathRequest>,
        ) -> Result<tonic::Response<super::GetPathResponse>, tonic::Status>;
        async fn set_as(
            &self,
            request: tonic::Request<super::SetAsRequest>,
        ) -> Result<tonic::Response<()>, tonic::Status>;
        async fn set_router_id(
            &self,
            request: tonic::Request<super::SetRouterIdRequest>,
        ) -> Result<tonic::Response<()>, tonic::Status>;
        async fn add_peer(
            &self,
            request: tonic::Request<super::AddPeerRequest>,
        ) -> Result<tonic::Response<()>, tonic::Status>;
        async fn delete_peer(
            &self,
            request: tonic::Request<super::DeletePeerRequest>,
        ) -> Result<tonic::Response<()>, tonic::Status>;
        async fn add_path(
            &self,
            request: tonic::Request<super::AddPathRequest>,
        ) -> Result<tonic::Response<super::AddPathResponse>, tonic::Status>;
        async fn delete_path(
            &self,
            request: tonic::Request<super::DeletePathRequest>,
        ) -> Result<tonic::Response<super::DeletePathResponse>, tonic::Status>;
    }
    #[derive(Debug)]
    pub struct BgpApiServer<T: BgpApi> {
        inner: _Inner<T>,
        accept_compression_encodings: EnabledCompressionEncodings,
        send_compression_encodings: EnabledCompressionEncodings,
    }
    struct _Inner<T>(Arc<T>);
    impl<T: BgpApi> BgpApiServer<T> {
        pub fn new(inner: T) -> Self {
            Self::from_arc(Arc::new(inner))
        }
        pub fn from_arc(inner: Arc<T>) -> Self {
            let inner = _Inner(inner);
            Self {
                inner,
                accept_compression_encodings: Default::default(),
                send_compression_encodings: Default::default(),
            }
        }
        pub fn with_interceptor<F>(
            inner: T,
            interceptor: F,
        ) -> InterceptedService<Self, F>
        where
            F: tonic::service::Interceptor,
        {
            InterceptedService::new(Self::new(inner), interceptor)
        }
        /// Enable decompressing requests with the given encoding.
        #[must_use]
        pub fn accept_compressed(mut self, encoding: CompressionEncoding) -> Self {
            self.accept_compression_encodings.enable(encoding);
            self
        }
        /// Compress responses with the given encoding, if the client supports it.
        #[must_use]
        pub fn send_compressed(mut self, encoding: CompressionEncoding) -> Self {
            self.send_compression_encodings.enable(encoding);
            self
        }
    }
    impl<T, B> tonic::codegen::Service<http::Request<B>> for BgpApiServer<T>
    where
        T: BgpApi,
        B: Body + Send + 'static,
        B::Error: Into<StdError> + Send + 'static,
    {
        type Response = http::Response<tonic::body::BoxBody>;
        type Error = std::convert::Infallible;
        type Future = BoxFuture<Self::Response, Self::Error>;
        fn poll_ready(
            &mut self,
            _cx: &mut Context<'_>,
        ) -> Poll<Result<(), Self::Error>> {
            Poll::Ready(Ok(()))
        }
        fn call(&mut self, req: http::Request<B>) -> Self::Future {
            let inner = self.inner.clone();
            match req.uri().path() {
                "/sart.BgpApi/Health" => {
                    #[allow(non_camel_case_types)]
                    struct HealthSvc<T: BgpApi>(pub Arc<T>);
                    impl<T: BgpApi> tonic::server::UnaryService<super::HealthRequest>
                    for HealthSvc<T> {
                        type Response = ();
                        type Future = BoxFuture<
                            tonic::Response<Self::Response>,
                            tonic::Status,
                        >;
                        fn call(
                            &mut self,
                            request: tonic::Request<super::HealthRequest>,
                        ) -> Self::Future {
                            let inner = self.0.clone();
                            let fut = async move { (*inner).health(request).await };
                            Box::pin(fut)
                        }
                    }
                    let accept_compression_encodings = self.accept_compression_encodings;
                    let send_compression_encodings = self.send_compression_encodings;
                    let inner = self.inner.clone();
                    let fut = async move {
                        let inner = inner.0;
                        let method = HealthSvc(inner);
                        let codec = tonic::codec::ProstCodec::default();
                        let mut grpc = tonic::server::Grpc::new(codec)
                            .apply_compression_config(
                                accept_compression_encodings,
                                send_compression_encodings,
                            );
                        let res = grpc.unary(method, req).await;
                        Ok(res)
                    };
                    Box::pin(fut)
                }
                "/sart.BgpApi/GetBgpInfo" => {
                    #[allow(non_camel_case_types)]
                    struct GetBgpInfoSvc<T: BgpApi>(pub Arc<T>);
                    impl<T: BgpApi> tonic::server::UnaryService<super::GetBgpInfoRequest>
                    for GetBgpInfoSvc<T> {
                        type Response = super::GetBgpInfoResponse;
                        type Future = BoxFuture<
                            tonic::Response<Self::Response>,
                            tonic::Status,
                        >;
                        fn call(
                            &mut self,
                            request: tonic::Request<super::GetBgpInfoRequest>,
                        ) -> Self::Future {
                            let inner = self.0.clone();
                            let fut = async move {
                                (*inner).get_bgp_info(request).await
                            };
                            Box::pin(fut)
                        }
                    }
                    let accept_compression_encodings = self.accept_compression_encodings;
                    let send_compression_encodings = self.send_compression_encodings;
                    let inner = self.inner.clone();
                    let fut = async move {
                        let inner = inner.0;
                        let method = GetBgpInfoSvc(inner);
                        let codec = tonic::codec::ProstCodec::default();
                        let mut grpc = tonic::server::Grpc::new(codec)
                            .apply_compression_config(
                                accept_compression_encodings,
                                send_compression_encodings,
                            );
                        let res = grpc.unary(method, req).await;
                        Ok(res)
                    };
                    Box::pin(fut)
                }
                "/sart.BgpApi/GetNeighbor" => {
                    #[allow(non_camel_case_types)]
                    struct GetNeighborSvc<T: BgpApi>(pub Arc<T>);
                    impl<
                        T: BgpApi,
                    > tonic::server::UnaryService<super::GetNeighborRequest>
                    for GetNeighborSvc<T> {
                        type Response = super::GetNeighborResponse;
                        type Future = BoxFuture<
                            tonic::Response<Self::Response>,
                            tonic::Status,
                        >;
                        fn call(
                            &mut self,
                            request: tonic::Request<super::GetNeighborRequest>,
                        ) -> Self::Future {
                            let inner = self.0.clone();
                            let fut = async move {
                                (*inner).get_neighbor(request).await
                            };
                            Box::pin(fut)
                        }
                    }
                    let accept_compression_encodings = self.accept_compression_encodings;
                    let send_compression_encodings = self.send_compression_encodings;
                    let inner = self.inner.clone();
                    let fut = async move {
                        let inner = inner.0;
                        let method = GetNeighborSvc(inner);
                        let codec = tonic::codec::ProstCodec::default();
                        let mut grpc = tonic::server::Grpc::new(codec)
                            .apply_compression_config(
                                accept_compression_encodings,
                                send_compression_encodings,
                            );
                        let res = grpc.unary(method, req).await;
                        Ok(res)
                    };
                    Box::pin(fut)
                }
                "/sart.BgpApi/GetPath" => {
                    #[allow(non_camel_case_types)]
                    struct GetPathSvc<T: BgpApi>(pub Arc<T>);
                    impl<T: BgpApi> tonic::server::UnaryService<super::GetPathRequest>
                    for GetPathSvc<T> {
                        type Response = super::GetPathResponse;
                        type Future = BoxFuture<
                            tonic::Response<Self::Response>,
                            tonic::Status,
                        >;
                        fn call(
                            &mut self,
                            request: tonic::Request<super::GetPathRequest>,
                        ) -> Self::Future {
                            let inner = self.0.clone();
                            let fut = async move { (*inner).get_path(request).await };
                            Box::pin(fut)
                        }
                    }
                    let accept_compression_encodings = self.accept_compression_encodings;
                    let send_compression_encodings = self.send_compression_encodings;
                    let inner = self.inner.clone();
                    let fut = async move {
                        let inner = inner.0;
                        let method = GetPathSvc(inner);
                        let codec = tonic::codec::ProstCodec::default();
                        let mut grpc = tonic::server::Grpc::new(codec)
                            .apply_compression_config(
                                accept_compression_encodings,
                                send_compression_encodings,
                            );
                        let res = grpc.unary(method, req).await;
                        Ok(res)
                    };
                    Box::pin(fut)
                }
                "/sart.BgpApi/SetAS" => {
                    #[allow(non_camel_case_types)]
                    struct SetASSvc<T: BgpApi>(pub Arc<T>);
                    impl<T: BgpApi> tonic::server::UnaryService<super::SetAsRequest>
                    for SetASSvc<T> {
                        type Response = ();
                        type Future = BoxFuture<
                            tonic::Response<Self::Response>,
                            tonic::Status,
                        >;
                        fn call(
                            &mut self,
                            request: tonic::Request<super::SetAsRequest>,
                        ) -> Self::Future {
                            let inner = self.0.clone();
                            let fut = async move { (*inner).set_as(request).await };
                            Box::pin(fut)
                        }
                    }
                    let accept_compression_encodings = self.accept_compression_encodings;
                    let send_compression_encodings = self.send_compression_encodings;
                    let inner = self.inner.clone();
                    let fut = async move {
                        let inner = inner.0;
                        let method = SetASSvc(inner);
                        let codec = tonic::codec::ProstCodec::default();
                        let mut grpc = tonic::server::Grpc::new(codec)
                            .apply_compression_config(
                                accept_compression_encodings,
                                send_compression_encodings,
                            );
                        let res = grpc.unary(method, req).await;
                        Ok(res)
                    };
                    Box::pin(fut)
                }
                "/sart.BgpApi/SetRouterId" => {
                    #[allow(non_camel_case_types)]
                    struct SetRouterIdSvc<T: BgpApi>(pub Arc<T>);
                    impl<
                        T: BgpApi,
                    > tonic::server::UnaryService<super::SetRouterIdRequest>
                    for SetRouterIdSvc<T> {
                        type Response = ();
                        type Future = BoxFuture<
                            tonic::Response<Self::Response>,
                            tonic::Status,
                        >;
                        fn call(
                            &mut self,
                            request: tonic::Request<super::SetRouterIdRequest>,
                        ) -> Self::Future {
                            let inner = self.0.clone();
                            let fut = async move {
                                (*inner).set_router_id(request).await
                            };
                            Box::pin(fut)
                        }
                    }
                    let accept_compression_encodings = self.accept_compression_encodings;
                    let send_compression_encodings = self.send_compression_encodings;
                    let inner = self.inner.clone();
                    let fut = async move {
                        let inner = inner.0;
                        let method = SetRouterIdSvc(inner);
                        let codec = tonic::codec::ProstCodec::default();
                        let mut grpc = tonic::server::Grpc::new(codec)
                            .apply_compression_config(
                                accept_compression_encodings,
                                send_compression_encodings,
                            );
                        let res = grpc.unary(method, req).await;
                        Ok(res)
                    };
                    Box::pin(fut)
                }
                "/sart.BgpApi/AddPeer" => {
                    #[allow(non_camel_case_types)]
                    struct AddPeerSvc<T: BgpApi>(pub Arc<T>);
                    impl<T: BgpApi> tonic::server::UnaryService<super::AddPeerRequest>
                    for AddPeerSvc<T> {
                        type Response = ();
                        type Future = BoxFuture<
                            tonic::Response<Self::Response>,
                            tonic::Status,
                        >;
                        fn call(
                            &mut self,
                            request: tonic::Request<super::AddPeerRequest>,
                        ) -> Self::Future {
                            let inner = self.0.clone();
                            let fut = async move { (*inner).add_peer(request).await };
                            Box::pin(fut)
                        }
                    }
                    let accept_compression_encodings = self.accept_compression_encodings;
                    let send_compression_encodings = self.send_compression_encodings;
                    let inner = self.inner.clone();
                    let fut = async move {
                        let inner = inner.0;
                        let method = AddPeerSvc(inner);
                        let codec = tonic::codec::ProstCodec::default();
                        let mut grpc = tonic::server::Grpc::new(codec)
                            .apply_compression_config(
                                accept_compression_encodings,
                                send_compression_encodings,
                            );
                        let res = grpc.unary(method, req).await;
                        Ok(res)
                    };
                    Box::pin(fut)
                }
                "/sart.BgpApi/DeletePeer" => {
                    #[allow(non_camel_case_types)]
                    struct DeletePeerSvc<T: BgpApi>(pub Arc<T>);
                    impl<T: BgpApi> tonic::server::UnaryService<super::DeletePeerRequest>
                    for DeletePeerSvc<T> {
                        type Response = ();
                        type Future = BoxFuture<
                            tonic::Response<Self::Response>,
                            tonic::Status,
                        >;
                        fn call(
                            &mut self,
                            request: tonic::Request<super::DeletePeerRequest>,
                        ) -> Self::Future {
                            let inner = self.0.clone();
                            let fut = async move { (*inner).delete_peer(request).await };
                            Box::pin(fut)
                        }
                    }
                    let accept_compression_encodings = self.accept_compression_encodings;
                    let send_compression_encodings = self.send_compression_encodings;
                    let inner = self.inner.clone();
                    let fut = async move {
                        let inner = inner.0;
                        let method = DeletePeerSvc(inner);
                        let codec = tonic::codec::ProstCodec::default();
                        let mut grpc = tonic::server::Grpc::new(codec)
                            .apply_compression_config(
                                accept_compression_encodings,
                                send_compression_encodings,
                            );
                        let res = grpc.unary(method, req).await;
                        Ok(res)
                    };
                    Box::pin(fut)
                }
                "/sart.BgpApi/AddPath" => {
                    #[allow(non_camel_case_types)]
                    struct AddPathSvc<T: BgpApi>(pub Arc<T>);
                    impl<T: BgpApi> tonic::server::UnaryService<super::AddPathRequest>
                    for AddPathSvc<T> {
                        type Response = super::AddPathResponse;
                        type Future = BoxFuture<
                            tonic::Response<Self::Response>,
                            tonic::Status,
                        >;
                        fn call(
                            &mut self,
                            request: tonic::Request<super::AddPathRequest>,
                        ) -> Self::Future {
                            let inner = self.0.clone();
                            let fut = async move { (*inner).add_path(request).await };
                            Box::pin(fut)
                        }
                    }
                    let accept_compression_encodings = self.accept_compression_encodings;
                    let send_compression_encodings = self.send_compression_encodings;
                    let inner = self.inner.clone();
                    let fut = async move {
                        let inner = inner.0;
                        let method = AddPathSvc(inner);
                        let codec = tonic::codec::ProstCodec::default();
                        let mut grpc = tonic::server::Grpc::new(codec)
                            .apply_compression_config(
                                accept_compression_encodings,
                                send_compression_encodings,
                            );
                        let res = grpc.unary(method, req).await;
                        Ok(res)
                    };
                    Box::pin(fut)
                }
                "/sart.BgpApi/DeletePath" => {
                    #[allow(non_camel_case_types)]
                    struct DeletePathSvc<T: BgpApi>(pub Arc<T>);
                    impl<T: BgpApi> tonic::server::UnaryService<super::DeletePathRequest>
                    for DeletePathSvc<T> {
                        type Response = super::DeletePathResponse;
                        type Future = BoxFuture<
                            tonic::Response<Self::Response>,
                            tonic::Status,
                        >;
                        fn call(
                            &mut self,
                            request: tonic::Request<super::DeletePathRequest>,
                        ) -> Self::Future {
                            let inner = self.0.clone();
                            let fut = async move { (*inner).delete_path(request).await };
                            Box::pin(fut)
                        }
                    }
                    let accept_compression_encodings = self.accept_compression_encodings;
                    let send_compression_encodings = self.send_compression_encodings;
                    let inner = self.inner.clone();
                    let fut = async move {
                        let inner = inner.0;
                        let method = DeletePathSvc(inner);
                        let codec = tonic::codec::ProstCodec::default();
                        let mut grpc = tonic::server::Grpc::new(codec)
                            .apply_compression_config(
                                accept_compression_encodings,
                                send_compression_encodings,
                            );
                        let res = grpc.unary(method, req).await;
                        Ok(res)
                    };
                    Box::pin(fut)
                }
                _ => {
                    Box::pin(async move {
                        Ok(
                            http::Response::builder()
                                .status(200)
                                .header("grpc-status", "12")
                                .header("content-type", "application/grpc")
                                .body(empty_body())
                                .unwrap(),
                        )
                    })
                }
            }
        }
    }
    impl<T: BgpApi> Clone for BgpApiServer<T> {
        fn clone(&self) -> Self {
            let inner = self.inner.clone();
            Self {
                inner,
                accept_compression_encodings: self.accept_compression_encodings,
                send_compression_encodings: self.send_compression_encodings,
            }
        }
    }
    impl<T: BgpApi> Clone for _Inner<T> {
        fn clone(&self) -> Self {
            Self(self.0.clone())
        }
    }
    impl<T: std::fmt::Debug> std::fmt::Debug for _Inner<T> {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "{:?}", self.0)
        }
    }
    impl<T: BgpApi> tonic::server::NamedService for BgpApiServer<T> {
        const NAME: &'static str = "sart.BgpApi";
    }
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetRouteRequest {
    #[prost(uint32, tag = "1")]
    pub table: u32,
    #[prost(enumeration = "IpVersion", tag = "2")]
    pub version: i32,
    #[prost(string, tag = "3")]
    pub destination: ::prost::alloc::string::String,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetRouteResponse {
    #[prost(message, optional, tag = "1")]
    pub route: ::core::option::Option<Route>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ListRoutesRequest {
    #[prost(uint32, tag = "1")]
    pub table: u32,
    #[prost(enumeration = "IpVersion", tag = "2")]
    pub version: i32,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ListRoutesResponse {
    #[prost(message, repeated, tag = "1")]
    pub routes: ::prost::alloc::vec::Vec<Route>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct AddRouteRequest {
    #[prost(uint32, tag = "1")]
    pub table: u32,
    #[prost(enumeration = "IpVersion", tag = "2")]
    pub version: i32,
    #[prost(message, optional, tag = "3")]
    pub route: ::core::option::Option<Route>,
    #[prost(bool, tag = "4")]
    pub replace: bool,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct DeleteRouteRequest {
    #[prost(uint32, tag = "1")]
    pub table: u32,
    #[prost(enumeration = "IpVersion", tag = "2")]
    pub version: i32,
    #[prost(string, tag = "3")]
    pub destination: ::prost::alloc::string::String,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct AddMultiPathRouteRequest {
    #[prost(uint32, tag = "1")]
    pub table: u32,
    #[prost(enumeration = "IpVersion", tag = "2")]
    pub version: i32,
    #[prost(string, tag = "3")]
    pub destination: ::prost::alloc::string::String,
    #[prost(message, repeated, tag = "4")]
    pub next_hops: ::prost::alloc::vec::Vec<NextHop>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct DeleteMultiPathRouteRequest {
    #[prost(uint32, tag = "1")]
    pub table: u32,
    #[prost(enumeration = "IpVersion", tag = "2")]
    pub version: i32,
    #[prost(string, tag = "3")]
    pub destination: ::prost::alloc::string::String,
    #[prost(string, repeated, tag = "4")]
    pub gateways: ::prost::alloc::vec::Vec<::prost::alloc::string::String>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Route {
    #[prost(uint32, tag = "1")]
    pub table: u32,
    #[prost(enumeration = "IpVersion", tag = "2")]
    pub version: i32,
    #[prost(string, tag = "3")]
    pub destination: ::prost::alloc::string::String,
    #[prost(enumeration = "Protocol", tag = "4")]
    pub protocol: i32,
    #[prost(enumeration = "Scope", tag = "5")]
    pub scope: i32,
    #[prost(enumeration = "Type", tag = "6")]
    pub r#type: i32,
    #[prost(message, repeated, tag = "7")]
    pub next_hops: ::prost::alloc::vec::Vec<NextHop>,
    #[prost(string, tag = "8")]
    pub source: ::prost::alloc::string::String,
    #[prost(enumeration = "AdministrativeDistance", tag = "9")]
    pub ad: i32,
    #[prost(uint32, tag = "10")]
    pub priority: u32,
    #[prost(bool, tag = "11")]
    pub ibgp: bool,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct NextHop {
    #[prost(string, tag = "1")]
    pub gateway: ::prost::alloc::string::String,
    #[prost(uint32, tag = "2")]
    pub weight: u32,
    #[prost(enumeration = "next_hop::NextHopFlags", tag = "3")]
    pub flags: i32,
    #[prost(uint32, tag = "4")]
    pub interface: u32,
}
/// Nested message and enum types in `NextHop`.
pub mod next_hop {
    #[derive(
        Clone,
        Copy,
        Debug,
        PartialEq,
        Eq,
        Hash,
        PartialOrd,
        Ord,
        ::prost::Enumeration
    )]
    #[repr(i32)]
    pub enum NextHopFlags {
        Empty = 0,
        Dead = 1,
        Pervasive = 2,
        Onlink = 3,
        Offload = 4,
        Linkdown = 16,
        Unresolved = 32,
    }
    impl NextHopFlags {
        /// String value of the enum field names used in the ProtoBuf definition.
        ///
        /// The values are not transformed in any way and thus are considered stable
        /// (if the ProtoBuf definition does not change) and safe for programmatic use.
        pub fn as_str_name(&self) -> &'static str {
            match self {
                NextHopFlags::Empty => "EMPTY",
                NextHopFlags::Dead => "DEAD",
                NextHopFlags::Pervasive => "PERVASIVE",
                NextHopFlags::Onlink => "ONLINK",
                NextHopFlags::Offload => "OFFLOAD",
                NextHopFlags::Linkdown => "LINKDOWN",
                NextHopFlags::Unresolved => "UNRESOLVED",
            }
        }
    }
}
/// message
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
#[repr(i32)]
pub enum IpVersion {
    Unkown = 0,
    V4 = 2,
    V6 = 10,
}
impl IpVersion {
    /// String value of the enum field names used in the ProtoBuf definition.
    ///
    /// The values are not transformed in any way and thus are considered stable
    /// (if the ProtoBuf definition does not change) and safe for programmatic use.
    pub fn as_str_name(&self) -> &'static str {
        match self {
            IpVersion::Unkown => "Unkown",
            IpVersion::V4 => "V4",
            IpVersion::V6 => "V6",
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
#[repr(i32)]
pub enum AdministrativeDistance {
    AdConnected = 0,
    AdStatic = 1,
    Adebgp = 20,
    Adospf = 110,
    Adrip = 120,
    Adibgp = 200,
}
impl AdministrativeDistance {
    /// String value of the enum field names used in the ProtoBuf definition.
    ///
    /// The values are not transformed in any way and thus are considered stable
    /// (if the ProtoBuf definition does not change) and safe for programmatic use.
    pub fn as_str_name(&self) -> &'static str {
        match self {
            AdministrativeDistance::AdConnected => "ADConnected",
            AdministrativeDistance::AdStatic => "ADStatic",
            AdministrativeDistance::Adebgp => "ADEBGP",
            AdministrativeDistance::Adospf => "ADOSPF",
            AdministrativeDistance::Adrip => "ADRIP",
            AdministrativeDistance::Adibgp => "ADIBGP",
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
#[repr(i32)]
pub enum Protocol {
    Unspec = 0,
    Redirect = 1,
    Kernel = 2,
    Boot = 3,
    Static = 4,
    Bgp = 186,
    IsIs = 187,
    Ospf = 188,
    Rip = 189,
}
impl Protocol {
    /// String value of the enum field names used in the ProtoBuf definition.
    ///
    /// The values are not transformed in any way and thus are considered stable
    /// (if the ProtoBuf definition does not change) and safe for programmatic use.
    pub fn as_str_name(&self) -> &'static str {
        match self {
            Protocol::Unspec => "Unspec",
            Protocol::Redirect => "Redirect",
            Protocol::Kernel => "Kernel",
            Protocol::Boot => "Boot",
            Protocol::Static => "Static",
            Protocol::Bgp => "Bgp",
            Protocol::IsIs => "IsIs",
            Protocol::Ospf => "Ospf",
            Protocol::Rip => "Rip",
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
#[repr(i32)]
pub enum Type {
    UnspecType = 0,
    Unicast = 1,
    Local = 2,
    Broadcast = 3,
    Anycast = 4,
    Multicast = 5,
    Blackhole = 6,
    Unreachable = 7,
    Prohibit = 8,
    Throw = 9,
    Nat = 10,
}
impl Type {
    /// String value of the enum field names used in the ProtoBuf definition.
    ///
    /// The values are not transformed in any way and thus are considered stable
    /// (if the ProtoBuf definition does not change) and safe for programmatic use.
    pub fn as_str_name(&self) -> &'static str {
        match self {
            Type::UnspecType => "UnspecType",
            Type::Unicast => "Unicast",
            Type::Local => "Local",
            Type::Broadcast => "Broadcast",
            Type::Anycast => "Anycast",
            Type::Multicast => "Multicast",
            Type::Blackhole => "Blackhole",
            Type::Unreachable => "Unreachable",
            Type::Prohibit => "Prohibit",
            Type::Throw => "Throw",
            Type::Nat => "Nat",
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
#[repr(i32)]
pub enum Scope {
    Universe = 0,
    Site = 200,
    Link = 253,
    Host = 254,
    Nowhere = 255,
}
impl Scope {
    /// String value of the enum field names used in the ProtoBuf definition.
    ///
    /// The values are not transformed in any way and thus are considered stable
    /// (if the ProtoBuf definition does not change) and safe for programmatic use.
    pub fn as_str_name(&self) -> &'static str {
        match self {
            Scope::Universe => "Universe",
            Scope::Site => "Site",
            Scope::Link => "Link",
            Scope::Host => "Host",
            Scope::Nowhere => "Nowhere",
        }
    }
}
/// Generated client implementations.
pub mod fib_api_client {
    #![allow(unused_variables, dead_code, missing_docs, clippy::let_unit_value)]
    use tonic::codegen::*;
    use tonic::codegen::http::Uri;
    #[derive(Debug, Clone)]
    pub struct FibApiClient<T> {
        inner: tonic::client::Grpc<T>,
    }
    impl FibApiClient<tonic::transport::Channel> {
        /// Attempt to create a new client by connecting to a given endpoint.
        pub async fn connect<D>(dst: D) -> Result<Self, tonic::transport::Error>
        where
            D: std::convert::TryInto<tonic::transport::Endpoint>,
            D::Error: Into<StdError>,
        {
            let conn = tonic::transport::Endpoint::new(dst)?.connect().await?;
            Ok(Self::new(conn))
        }
    }
    impl<T> FibApiClient<T>
    where
        T: tonic::client::GrpcService<tonic::body::BoxBody>,
        T::Error: Into<StdError>,
        T::ResponseBody: Body<Data = Bytes> + Send + 'static,
        <T::ResponseBody as Body>::Error: Into<StdError> + Send,
    {
        pub fn new(inner: T) -> Self {
            let inner = tonic::client::Grpc::new(inner);
            Self { inner }
        }
        pub fn with_origin(inner: T, origin: Uri) -> Self {
            let inner = tonic::client::Grpc::with_origin(inner, origin);
            Self { inner }
        }
        pub fn with_interceptor<F>(
            inner: T,
            interceptor: F,
        ) -> FibApiClient<InterceptedService<T, F>>
        where
            F: tonic::service::Interceptor,
            T::ResponseBody: Default,
            T: tonic::codegen::Service<
                http::Request<tonic::body::BoxBody>,
                Response = http::Response<
                    <T as tonic::client::GrpcService<tonic::body::BoxBody>>::ResponseBody,
                >,
            >,
            <T as tonic::codegen::Service<
                http::Request<tonic::body::BoxBody>,
            >>::Error: Into<StdError> + Send + Sync,
        {
            FibApiClient::new(InterceptedService::new(inner, interceptor))
        }
        /// Compress requests with the given encoding.
        ///
        /// This requires the server to support it otherwise it might respond with an
        /// error.
        #[must_use]
        pub fn send_compressed(mut self, encoding: CompressionEncoding) -> Self {
            self.inner = self.inner.send_compressed(encoding);
            self
        }
        /// Enable decompressing responses.
        #[must_use]
        pub fn accept_compressed(mut self, encoding: CompressionEncoding) -> Self {
            self.inner = self.inner.accept_compressed(encoding);
            self
        }
        pub async fn get_route(
            &mut self,
            request: impl tonic::IntoRequest<super::GetRouteRequest>,
        ) -> Result<tonic::Response<super::GetRouteResponse>, tonic::Status> {
            self.inner
                .ready()
                .await
                .map_err(|e| {
                    tonic::Status::new(
                        tonic::Code::Unknown,
                        format!("Service was not ready: {}", e.into()),
                    )
                })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static("/sart.FibApi/GetRoute");
            self.inner.unary(request.into_request(), path, codec).await
        }
        pub async fn list_routes(
            &mut self,
            request: impl tonic::IntoRequest<super::ListRoutesRequest>,
        ) -> Result<tonic::Response<super::ListRoutesResponse>, tonic::Status> {
            self.inner
                .ready()
                .await
                .map_err(|e| {
                    tonic::Status::new(
                        tonic::Code::Unknown,
                        format!("Service was not ready: {}", e.into()),
                    )
                })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static("/sart.FibApi/ListRoutes");
            self.inner.unary(request.into_request(), path, codec).await
        }
        pub async fn add_route(
            &mut self,
            request: impl tonic::IntoRequest<super::AddRouteRequest>,
        ) -> Result<tonic::Response<()>, tonic::Status> {
            self.inner
                .ready()
                .await
                .map_err(|e| {
                    tonic::Status::new(
                        tonic::Code::Unknown,
                        format!("Service was not ready: {}", e.into()),
                    )
                })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static("/sart.FibApi/AddRoute");
            self.inner.unary(request.into_request(), path, codec).await
        }
        pub async fn delete_route(
            &mut self,
            request: impl tonic::IntoRequest<super::DeleteRouteRequest>,
        ) -> Result<tonic::Response<()>, tonic::Status> {
            self.inner
                .ready()
                .await
                .map_err(|e| {
                    tonic::Status::new(
                        tonic::Code::Unknown,
                        format!("Service was not ready: {}", e.into()),
                    )
                })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static("/sart.FibApi/DeleteRoute");
            self.inner.unary(request.into_request(), path, codec).await
        }
        pub async fn add_multi_path_route(
            &mut self,
            request: impl tonic::IntoRequest<super::AddMultiPathRouteRequest>,
        ) -> Result<tonic::Response<()>, tonic::Status> {
            self.inner
                .ready()
                .await
                .map_err(|e| {
                    tonic::Status::new(
                        tonic::Code::Unknown,
                        format!("Service was not ready: {}", e.into()),
                    )
                })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static(
                "/sart.FibApi/AddMultiPathRoute",
            );
            self.inner.unary(request.into_request(), path, codec).await
        }
        pub async fn delete_multi_path_route(
            &mut self,
            request: impl tonic::IntoRequest<super::DeleteMultiPathRouteRequest>,
        ) -> Result<tonic::Response<()>, tonic::Status> {
            self.inner
                .ready()
                .await
                .map_err(|e| {
                    tonic::Status::new(
                        tonic::Code::Unknown,
                        format!("Service was not ready: {}", e.into()),
                    )
                })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static(
                "/sart.FibApi/DeleteMultiPathRoute",
            );
            self.inner.unary(request.into_request(), path, codec).await
        }
    }
}
/// Generated server implementations.
pub mod fib_api_server {
    #![allow(unused_variables, dead_code, missing_docs, clippy::let_unit_value)]
    use tonic::codegen::*;
    /// Generated trait containing gRPC methods that should be implemented for use with FibApiServer.
    #[async_trait]
    pub trait FibApi: Send + Sync + 'static {
        async fn get_route(
            &self,
            request: tonic::Request<super::GetRouteRequest>,
        ) -> Result<tonic::Response<super::GetRouteResponse>, tonic::Status>;
        async fn list_routes(
            &self,
            request: tonic::Request<super::ListRoutesRequest>,
        ) -> Result<tonic::Response<super::ListRoutesResponse>, tonic::Status>;
        async fn add_route(
            &self,
            request: tonic::Request<super::AddRouteRequest>,
        ) -> Result<tonic::Response<()>, tonic::Status>;
        async fn delete_route(
            &self,
            request: tonic::Request<super::DeleteRouteRequest>,
        ) -> Result<tonic::Response<()>, tonic::Status>;
        async fn add_multi_path_route(
            &self,
            request: tonic::Request<super::AddMultiPathRouteRequest>,
        ) -> Result<tonic::Response<()>, tonic::Status>;
        async fn delete_multi_path_route(
            &self,
            request: tonic::Request<super::DeleteMultiPathRouteRequest>,
        ) -> Result<tonic::Response<()>, tonic::Status>;
    }
    #[derive(Debug)]
    pub struct FibApiServer<T: FibApi> {
        inner: _Inner<T>,
        accept_compression_encodings: EnabledCompressionEncodings,
        send_compression_encodings: EnabledCompressionEncodings,
    }
    struct _Inner<T>(Arc<T>);
    impl<T: FibApi> FibApiServer<T> {
        pub fn new(inner: T) -> Self {
            Self::from_arc(Arc::new(inner))
        }
        pub fn from_arc(inner: Arc<T>) -> Self {
            let inner = _Inner(inner);
            Self {
                inner,
                accept_compression_encodings: Default::default(),
                send_compression_encodings: Default::default(),
            }
        }
        pub fn with_interceptor<F>(
            inner: T,
            interceptor: F,
        ) -> InterceptedService<Self, F>
        where
            F: tonic::service::Interceptor,
        {
            InterceptedService::new(Self::new(inner), interceptor)
        }
        /// Enable decompressing requests with the given encoding.
        #[must_use]
        pub fn accept_compressed(mut self, encoding: CompressionEncoding) -> Self {
            self.accept_compression_encodings.enable(encoding);
            self
        }
        /// Compress responses with the given encoding, if the client supports it.
        #[must_use]
        pub fn send_compressed(mut self, encoding: CompressionEncoding) -> Self {
            self.send_compression_encodings.enable(encoding);
            self
        }
    }
    impl<T, B> tonic::codegen::Service<http::Request<B>> for FibApiServer<T>
    where
        T: FibApi,
        B: Body + Send + 'static,
        B::Error: Into<StdError> + Send + 'static,
    {
        type Response = http::Response<tonic::body::BoxBody>;
        type Error = std::convert::Infallible;
        type Future = BoxFuture<Self::Response, Self::Error>;
        fn poll_ready(
            &mut self,
            _cx: &mut Context<'_>,
        ) -> Poll<Result<(), Self::Error>> {
            Poll::Ready(Ok(()))
        }
        fn call(&mut self, req: http::Request<B>) -> Self::Future {
            let inner = self.inner.clone();
            match req.uri().path() {
                "/sart.FibApi/GetRoute" => {
                    #[allow(non_camel_case_types)]
                    struct GetRouteSvc<T: FibApi>(pub Arc<T>);
                    impl<T: FibApi> tonic::server::UnaryService<super::GetRouteRequest>
                    for GetRouteSvc<T> {
                        type Response = super::GetRouteResponse;
                        type Future = BoxFuture<
                            tonic::Response<Self::Response>,
                            tonic::Status,
                        >;
                        fn call(
                            &mut self,
                            request: tonic::Request<super::GetRouteRequest>,
                        ) -> Self::Future {
                            let inner = self.0.clone();
                            let fut = async move { (*inner).get_route(request).await };
                            Box::pin(fut)
                        }
                    }
                    let accept_compression_encodings = self.accept_compression_encodings;
                    let send_compression_encodings = self.send_compression_encodings;
                    let inner = self.inner.clone();
                    let fut = async move {
                        let inner = inner.0;
                        let method = GetRouteSvc(inner);
                        let codec = tonic::codec::ProstCodec::default();
                        let mut grpc = tonic::server::Grpc::new(codec)
                            .apply_compression_config(
                                accept_compression_encodings,
                                send_compression_encodings,
                            );
                        let res = grpc.unary(method, req).await;
                        Ok(res)
                    };
                    Box::pin(fut)
                }
                "/sart.FibApi/ListRoutes" => {
                    #[allow(non_camel_case_types)]
                    struct ListRoutesSvc<T: FibApi>(pub Arc<T>);
                    impl<T: FibApi> tonic::server::UnaryService<super::ListRoutesRequest>
                    for ListRoutesSvc<T> {
                        type Response = super::ListRoutesResponse;
                        type Future = BoxFuture<
                            tonic::Response<Self::Response>,
                            tonic::Status,
                        >;
                        fn call(
                            &mut self,
                            request: tonic::Request<super::ListRoutesRequest>,
                        ) -> Self::Future {
                            let inner = self.0.clone();
                            let fut = async move { (*inner).list_routes(request).await };
                            Box::pin(fut)
                        }
                    }
                    let accept_compression_encodings = self.accept_compression_encodings;
                    let send_compression_encodings = self.send_compression_encodings;
                    let inner = self.inner.clone();
                    let fut = async move {
                        let inner = inner.0;
                        let method = ListRoutesSvc(inner);
                        let codec = tonic::codec::ProstCodec::default();
                        let mut grpc = tonic::server::Grpc::new(codec)
                            .apply_compression_config(
                                accept_compression_encodings,
                                send_compression_encodings,
                            );
                        let res = grpc.unary(method, req).await;
                        Ok(res)
                    };
                    Box::pin(fut)
                }
                "/sart.FibApi/AddRoute" => {
                    #[allow(non_camel_case_types)]
                    struct AddRouteSvc<T: FibApi>(pub Arc<T>);
                    impl<T: FibApi> tonic::server::UnaryService<super::AddRouteRequest>
                    for AddRouteSvc<T> {
                        type Response = ();
                        type Future = BoxFuture<
                            tonic::Response<Self::Response>,
                            tonic::Status,
                        >;
                        fn call(
                            &mut self,
                            request: tonic::Request<super::AddRouteRequest>,
                        ) -> Self::Future {
                            let inner = self.0.clone();
                            let fut = async move { (*inner).add_route(request).await };
                            Box::pin(fut)
                        }
                    }
                    let accept_compression_encodings = self.accept_compression_encodings;
                    let send_compression_encodings = self.send_compression_encodings;
                    let inner = self.inner.clone();
                    let fut = async move {
                        let inner = inner.0;
                        let method = AddRouteSvc(inner);
                        let codec = tonic::codec::ProstCodec::default();
                        let mut grpc = tonic::server::Grpc::new(codec)
                            .apply_compression_config(
                                accept_compression_encodings,
                                send_compression_encodings,
                            );
                        let res = grpc.unary(method, req).await;
                        Ok(res)
                    };
                    Box::pin(fut)
                }
                "/sart.FibApi/DeleteRoute" => {
                    #[allow(non_camel_case_types)]
                    struct DeleteRouteSvc<T: FibApi>(pub Arc<T>);
                    impl<
                        T: FibApi,
                    > tonic::server::UnaryService<super::DeleteRouteRequest>
                    for DeleteRouteSvc<T> {
                        type Response = ();
                        type Future = BoxFuture<
                            tonic::Response<Self::Response>,
                            tonic::Status,
                        >;
                        fn call(
                            &mut self,
                            request: tonic::Request<super::DeleteRouteRequest>,
                        ) -> Self::Future {
                            let inner = self.0.clone();
                            let fut = async move {
                                (*inner).delete_route(request).await
                            };
                            Box::pin(fut)
                        }
                    }
                    let accept_compression_encodings = self.accept_compression_encodings;
                    let send_compression_encodings = self.send_compression_encodings;
                    let inner = self.inner.clone();
                    let fut = async move {
                        let inner = inner.0;
                        let method = DeleteRouteSvc(inner);
                        let codec = tonic::codec::ProstCodec::default();
                        let mut grpc = tonic::server::Grpc::new(codec)
                            .apply_compression_config(
                                accept_compression_encodings,
                                send_compression_encodings,
                            );
                        let res = grpc.unary(method, req).await;
                        Ok(res)
                    };
                    Box::pin(fut)
                }
                "/sart.FibApi/AddMultiPathRoute" => {
                    #[allow(non_camel_case_types)]
                    struct AddMultiPathRouteSvc<T: FibApi>(pub Arc<T>);
                    impl<
                        T: FibApi,
                    > tonic::server::UnaryService<super::AddMultiPathRouteRequest>
                    for AddMultiPathRouteSvc<T> {
                        type Response = ();
                        type Future = BoxFuture<
                            tonic::Response<Self::Response>,
                            tonic::Status,
                        >;
                        fn call(
                            &mut self,
                            request: tonic::Request<super::AddMultiPathRouteRequest>,
                        ) -> Self::Future {
                            let inner = self.0.clone();
                            let fut = async move {
                                (*inner).add_multi_path_route(request).await
                            };
                            Box::pin(fut)
                        }
                    }
                    let accept_compression_encodings = self.accept_compression_encodings;
                    let send_compression_encodings = self.send_compression_encodings;
                    let inner = self.inner.clone();
                    let fut = async move {
                        let inner = inner.0;
                        let method = AddMultiPathRouteSvc(inner);
                        let codec = tonic::codec::ProstCodec::default();
                        let mut grpc = tonic::server::Grpc::new(codec)
                            .apply_compression_config(
                                accept_compression_encodings,
                                send_compression_encodings,
                            );
                        let res = grpc.unary(method, req).await;
                        Ok(res)
                    };
                    Box::pin(fut)
                }
                "/sart.FibApi/DeleteMultiPathRoute" => {
                    #[allow(non_camel_case_types)]
                    struct DeleteMultiPathRouteSvc<T: FibApi>(pub Arc<T>);
                    impl<
                        T: FibApi,
                    > tonic::server::UnaryService<super::DeleteMultiPathRouteRequest>
                    for DeleteMultiPathRouteSvc<T> {
                        type Response = ();
                        type Future = BoxFuture<
                            tonic::Response<Self::Response>,
                            tonic::Status,
                        >;
                        fn call(
                            &mut self,
                            request: tonic::Request<super::DeleteMultiPathRouteRequest>,
                        ) -> Self::Future {
                            let inner = self.0.clone();
                            let fut = async move {
                                (*inner).delete_multi_path_route(request).await
                            };
                            Box::pin(fut)
                        }
                    }
                    let accept_compression_encodings = self.accept_compression_encodings;
                    let send_compression_encodings = self.send_compression_encodings;
                    let inner = self.inner.clone();
                    let fut = async move {
                        let inner = inner.0;
                        let method = DeleteMultiPathRouteSvc(inner);
                        let codec = tonic::codec::ProstCodec::default();
                        let mut grpc = tonic::server::Grpc::new(codec)
                            .apply_compression_config(
                                accept_compression_encodings,
                                send_compression_encodings,
                            );
                        let res = grpc.unary(method, req).await;
                        Ok(res)
                    };
                    Box::pin(fut)
                }
                _ => {
                    Box::pin(async move {
                        Ok(
                            http::Response::builder()
                                .status(200)
                                .header("grpc-status", "12")
                                .header("content-type", "application/grpc")
                                .body(empty_body())
                                .unwrap(),
                        )
                    })
                }
            }
        }
    }
    impl<T: FibApi> Clone for FibApiServer<T> {
        fn clone(&self) -> Self {
            let inner = self.inner.clone();
            Self {
                inner,
                accept_compression_encodings: self.accept_compression_encodings,
                send_compression_encodings: self.send_compression_encodings,
            }
        }
    }
    impl<T: FibApi> Clone for _Inner<T> {
        fn clone(&self) -> Self {
            Self(self.0.clone())
        }
    }
    impl<T: std::fmt::Debug> std::fmt::Debug for _Inner<T> {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "{:?}", self.0)
        }
    }
    impl<T: FibApi> tonic::server::NamedService for FibApiServer<T> {
        const NAME: &'static str = "sart.FibApi";
    }
}
