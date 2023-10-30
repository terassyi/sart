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
pub struct ListNeighborRequest {}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ListNeighborResponse {
    #[prost(message, repeated, tag = "1")]
    pub peers: ::prost::alloc::vec::Vec<Peer>,
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
pub struct GetNeighborPathRequest {
    #[prost(enumeration = "get_neighbor_path_request::Kind", tag = "1")]
    pub kind: i32,
    #[prost(string, tag = "2")]
    pub addr: ::prost::alloc::string::String,
    #[prost(message, optional, tag = "3")]
    pub family: ::core::option::Option<AddressFamily>,
}
/// Nested message and enum types in `GetNeighborPathRequest`.
pub mod get_neighbor_path_request {
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
    pub enum Kind {
        Unknown = 0,
        In = 1,
        Out = 2,
    }
    impl Kind {
        /// String value of the enum field names used in the ProtoBuf definition.
        ///
        /// The values are not transformed in any way and thus are considered stable
        /// (if the ProtoBuf definition does not change) and safe for programmatic use.
        pub fn as_str_name(&self) -> &'static str {
            match self {
                Kind::Unknown => "UNKNOWN",
                Kind::In => "IN",
                Kind::Out => "OUT",
            }
        }
        /// Creates an enum from field names used in the ProtoBuf definition.
        pub fn from_str_name(value: &str) -> ::core::option::Option<Self> {
            match value {
                "UNKNOWN" => Some(Self::Unknown),
                "IN" => Some(Self::In),
                "OUT" => Some(Self::Out),
                _ => None,
            }
        }
    }
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetNeighborPathResponse {
    #[prost(message, repeated, tag = "1")]
    pub paths: ::prost::alloc::vec::Vec<Path>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetPathByPrefixRequest {
    #[prost(string, tag = "1")]
    pub prefix: ::prost::alloc::string::String,
    #[prost(message, optional, tag = "2")]
    pub family: ::core::option::Option<AddressFamily>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetPathByPrefixResponse {
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
        /// Creates an enum from field names used in the ProtoBuf definition.
        pub fn from_str_name(value: &str) -> ::core::option::Option<Self> {
            match value {
                "AFI_UNKNOWN" => Some(Self::Unknown),
                "AFI_IP4" => Some(Self::Ip4),
                "AFI_IP6" => Some(Self::Ip6),
                _ => None,
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
        /// Creates an enum from field names used in the ProtoBuf definition.
        pub fn from_str_name(value: &str) -> ::core::option::Option<Self> {
            match value {
                "SAFI_UNKNOWN" => Some(Self::Unknown),
                "SAFI_UNICAST" => Some(Self::Unicast),
                "SAFI_MULTICAST" => Some(Self::Multicast),
                _ => None,
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
    #[prost(string, tag = "12")]
    pub name: ::prost::alloc::string::String,
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
        /// Creates an enum from field names used in the ProtoBuf definition.
        pub fn from_str_name(value: &str) -> ::core::option::Option<Self> {
            match value {
                "UNKNOWN" => Some(Self::Unknown),
                "IDLE" => Some(Self::Idle),
                "CONNECT" => Some(Self::Connect),
                "ACTIVE" => Some(Self::Active),
                "OPEN_SENT" => Some(Self::OpenSent),
                "OPEN_CONFIRM" => Some(Self::OpenConfirm),
                "ESTABLISHED" => Some(Self::Established),
                _ => None,
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
        /// Creates an enum from field names used in the ProtoBuf definition.
        pub fn from_str_name(value: &str) -> ::core::option::Option<Self> {
            match value {
                "UNKNOWN" => Some(Self::Unknown),
                "AS_SET" => Some(Self::AsSet),
                "AS_SEQUENCE" => Some(Self::AsSequence),
                _ => None,
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
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ExportPeerRequest {
    #[prost(message, optional, tag = "1")]
    pub peer: ::core::option::Option<Peer>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ExportPeerStateRequest {
    #[prost(uint32, tag = "1")]
    pub asn: u32,
    #[prost(string, tag = "2")]
    pub addr: ::prost::alloc::string::String,
    #[prost(enumeration = "export_peer_state_request::State", tag = "3")]
    pub state: i32,
    #[prost(string, tag = "4")]
    pub name: ::prost::alloc::string::String,
}
/// Nested message and enum types in `ExportPeerStateRequest`.
pub mod export_peer_state_request {
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
        /// Creates an enum from field names used in the ProtoBuf definition.
        pub fn from_str_name(value: &str) -> ::core::option::Option<Self> {
            match value {
                "UNKNOWN" => Some(Self::Unknown),
                "IDLE" => Some(Self::Idle),
                "CONNECT" => Some(Self::Connect),
                "ACTIVE" => Some(Self::Active),
                "OPEN_SENT" => Some(Self::OpenSent),
                "OPEN_CONFIRM" => Some(Self::OpenConfirm),
                "ESTABLISHED" => Some(Self::Established),
                _ => None,
            }
        }
    }
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
            D: TryInto<tonic::transport::Endpoint>,
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
        /// Limits the maximum size of a decoded message.
        ///
        /// Default: `4MB`
        #[must_use]
        pub fn max_decoding_message_size(mut self, limit: usize) -> Self {
            self.inner = self.inner.max_decoding_message_size(limit);
            self
        }
        /// Limits the maximum size of an encoded message.
        ///
        /// Default: `usize::MAX`
        #[must_use]
        pub fn max_encoding_message_size(mut self, limit: usize) -> Self {
            self.inner = self.inner.max_encoding_message_size(limit);
            self
        }
        pub async fn health(
            &mut self,
            request: impl tonic::IntoRequest<super::HealthRequest>,
        ) -> std::result::Result<tonic::Response<()>, tonic::Status> {
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
            let path = http::uri::PathAndQuery::from_static("/sart.v1.BgpApi/Health");
            let mut req = request.into_request();
            req.extensions_mut().insert(GrpcMethod::new("sart.v1.BgpApi", "Health"));
            self.inner.unary(req, path, codec).await
        }
        pub async fn get_bgp_info(
            &mut self,
            request: impl tonic::IntoRequest<super::GetBgpInfoRequest>,
        ) -> std::result::Result<
            tonic::Response<super::GetBgpInfoResponse>,
            tonic::Status,
        > {
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
                "/sart.v1.BgpApi/GetBgpInfo",
            );
            let mut req = request.into_request();
            req.extensions_mut().insert(GrpcMethod::new("sart.v1.BgpApi", "GetBgpInfo"));
            self.inner.unary(req, path, codec).await
        }
        pub async fn get_neighbor(
            &mut self,
            request: impl tonic::IntoRequest<super::GetNeighborRequest>,
        ) -> std::result::Result<
            tonic::Response<super::GetNeighborResponse>,
            tonic::Status,
        > {
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
                "/sart.v1.BgpApi/GetNeighbor",
            );
            let mut req = request.into_request();
            req.extensions_mut()
                .insert(GrpcMethod::new("sart.v1.BgpApi", "GetNeighbor"));
            self.inner.unary(req, path, codec).await
        }
        pub async fn list_neighbor(
            &mut self,
            request: impl tonic::IntoRequest<super::ListNeighborRequest>,
        ) -> std::result::Result<
            tonic::Response<super::ListNeighborResponse>,
            tonic::Status,
        > {
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
                "/sart.v1.BgpApi/ListNeighbor",
            );
            let mut req = request.into_request();
            req.extensions_mut()
                .insert(GrpcMethod::new("sart.v1.BgpApi", "ListNeighbor"));
            self.inner.unary(req, path, codec).await
        }
        pub async fn get_path(
            &mut self,
            request: impl tonic::IntoRequest<super::GetPathRequest>,
        ) -> std::result::Result<
            tonic::Response<super::GetPathResponse>,
            tonic::Status,
        > {
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
            let path = http::uri::PathAndQuery::from_static("/sart.v1.BgpApi/GetPath");
            let mut req = request.into_request();
            req.extensions_mut().insert(GrpcMethod::new("sart.v1.BgpApi", "GetPath"));
            self.inner.unary(req, path, codec).await
        }
        pub async fn get_neighbor_path(
            &mut self,
            request: impl tonic::IntoRequest<super::GetNeighborPathRequest>,
        ) -> std::result::Result<
            tonic::Response<super::GetNeighborPathResponse>,
            tonic::Status,
        > {
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
                "/sart.v1.BgpApi/GetNeighborPath",
            );
            let mut req = request.into_request();
            req.extensions_mut()
                .insert(GrpcMethod::new("sart.v1.BgpApi", "GetNeighborPath"));
            self.inner.unary(req, path, codec).await
        }
        pub async fn get_path_by_prefix(
            &mut self,
            request: impl tonic::IntoRequest<super::GetPathByPrefixRequest>,
        ) -> std::result::Result<
            tonic::Response<super::GetPathByPrefixResponse>,
            tonic::Status,
        > {
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
                "/sart.v1.BgpApi/GetPathByPrefix",
            );
            let mut req = request.into_request();
            req.extensions_mut()
                .insert(GrpcMethod::new("sart.v1.BgpApi", "GetPathByPrefix"));
            self.inner.unary(req, path, codec).await
        }
        pub async fn set_as(
            &mut self,
            request: impl tonic::IntoRequest<super::SetAsRequest>,
        ) -> std::result::Result<tonic::Response<()>, tonic::Status> {
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
            let path = http::uri::PathAndQuery::from_static("/sart.v1.BgpApi/SetAS");
            let mut req = request.into_request();
            req.extensions_mut().insert(GrpcMethod::new("sart.v1.BgpApi", "SetAS"));
            self.inner.unary(req, path, codec).await
        }
        pub async fn set_router_id(
            &mut self,
            request: impl tonic::IntoRequest<super::SetRouterIdRequest>,
        ) -> std::result::Result<tonic::Response<()>, tonic::Status> {
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
                "/sart.v1.BgpApi/SetRouterId",
            );
            let mut req = request.into_request();
            req.extensions_mut()
                .insert(GrpcMethod::new("sart.v1.BgpApi", "SetRouterId"));
            self.inner.unary(req, path, codec).await
        }
        pub async fn add_peer(
            &mut self,
            request: impl tonic::IntoRequest<super::AddPeerRequest>,
        ) -> std::result::Result<tonic::Response<()>, tonic::Status> {
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
            let path = http::uri::PathAndQuery::from_static("/sart.v1.BgpApi/AddPeer");
            let mut req = request.into_request();
            req.extensions_mut().insert(GrpcMethod::new("sart.v1.BgpApi", "AddPeer"));
            self.inner.unary(req, path, codec).await
        }
        pub async fn delete_peer(
            &mut self,
            request: impl tonic::IntoRequest<super::DeletePeerRequest>,
        ) -> std::result::Result<tonic::Response<()>, tonic::Status> {
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
                "/sart.v1.BgpApi/DeletePeer",
            );
            let mut req = request.into_request();
            req.extensions_mut().insert(GrpcMethod::new("sart.v1.BgpApi", "DeletePeer"));
            self.inner.unary(req, path, codec).await
        }
        pub async fn add_path(
            &mut self,
            request: impl tonic::IntoRequest<super::AddPathRequest>,
        ) -> std::result::Result<
            tonic::Response<super::AddPathResponse>,
            tonic::Status,
        > {
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
            let path = http::uri::PathAndQuery::from_static("/sart.v1.BgpApi/AddPath");
            let mut req = request.into_request();
            req.extensions_mut().insert(GrpcMethod::new("sart.v1.BgpApi", "AddPath"));
            self.inner.unary(req, path, codec).await
        }
        pub async fn delete_path(
            &mut self,
            request: impl tonic::IntoRequest<super::DeletePathRequest>,
        ) -> std::result::Result<
            tonic::Response<super::DeletePathResponse>,
            tonic::Status,
        > {
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
                "/sart.v1.BgpApi/DeletePath",
            );
            let mut req = request.into_request();
            req.extensions_mut().insert(GrpcMethod::new("sart.v1.BgpApi", "DeletePath"));
            self.inner.unary(req, path, codec).await
        }
    }
}
/// Generated client implementations.
pub mod bgp_exporter_api_client {
    #![allow(unused_variables, dead_code, missing_docs, clippy::let_unit_value)]
    use tonic::codegen::*;
    use tonic::codegen::http::Uri;
    #[derive(Debug, Clone)]
    pub struct BgpExporterApiClient<T> {
        inner: tonic::client::Grpc<T>,
    }
    impl BgpExporterApiClient<tonic::transport::Channel> {
        /// Attempt to create a new client by connecting to a given endpoint.
        pub async fn connect<D>(dst: D) -> Result<Self, tonic::transport::Error>
        where
            D: TryInto<tonic::transport::Endpoint>,
            D::Error: Into<StdError>,
        {
            let conn = tonic::transport::Endpoint::new(dst)?.connect().await?;
            Ok(Self::new(conn))
        }
    }
    impl<T> BgpExporterApiClient<T>
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
        ) -> BgpExporterApiClient<InterceptedService<T, F>>
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
            BgpExporterApiClient::new(InterceptedService::new(inner, interceptor))
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
        /// Limits the maximum size of a decoded message.
        ///
        /// Default: `4MB`
        #[must_use]
        pub fn max_decoding_message_size(mut self, limit: usize) -> Self {
            self.inner = self.inner.max_decoding_message_size(limit);
            self
        }
        /// Limits the maximum size of an encoded message.
        ///
        /// Default: `usize::MAX`
        #[must_use]
        pub fn max_encoding_message_size(mut self, limit: usize) -> Self {
            self.inner = self.inner.max_encoding_message_size(limit);
            self
        }
        pub async fn export_peer(
            &mut self,
            request: impl tonic::IntoRequest<super::ExportPeerRequest>,
        ) -> std::result::Result<tonic::Response<()>, tonic::Status> {
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
                "/sart.v1.BgpExporterApi/ExportPeer",
            );
            let mut req = request.into_request();
            req.extensions_mut()
                .insert(GrpcMethod::new("sart.v1.BgpExporterApi", "ExportPeer"));
            self.inner.unary(req, path, codec).await
        }
        pub async fn export_peer_state(
            &mut self,
            request: impl tonic::IntoRequest<super::ExportPeerStateRequest>,
        ) -> std::result::Result<tonic::Response<()>, tonic::Status> {
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
                "/sart.v1.BgpExporterApi/ExportPeerState",
            );
            let mut req = request.into_request();
            req.extensions_mut()
                .insert(GrpcMethod::new("sart.v1.BgpExporterApi", "ExportPeerState"));
            self.inner.unary(req, path, codec).await
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
        ) -> std::result::Result<tonic::Response<()>, tonic::Status>;
        async fn get_bgp_info(
            &self,
            request: tonic::Request<super::GetBgpInfoRequest>,
        ) -> std::result::Result<
            tonic::Response<super::GetBgpInfoResponse>,
            tonic::Status,
        >;
        async fn get_neighbor(
            &self,
            request: tonic::Request<super::GetNeighborRequest>,
        ) -> std::result::Result<
            tonic::Response<super::GetNeighborResponse>,
            tonic::Status,
        >;
        async fn list_neighbor(
            &self,
            request: tonic::Request<super::ListNeighborRequest>,
        ) -> std::result::Result<
            tonic::Response<super::ListNeighborResponse>,
            tonic::Status,
        >;
        async fn get_path(
            &self,
            request: tonic::Request<super::GetPathRequest>,
        ) -> std::result::Result<tonic::Response<super::GetPathResponse>, tonic::Status>;
        async fn get_neighbor_path(
            &self,
            request: tonic::Request<super::GetNeighborPathRequest>,
        ) -> std::result::Result<
            tonic::Response<super::GetNeighborPathResponse>,
            tonic::Status,
        >;
        async fn get_path_by_prefix(
            &self,
            request: tonic::Request<super::GetPathByPrefixRequest>,
        ) -> std::result::Result<
            tonic::Response<super::GetPathByPrefixResponse>,
            tonic::Status,
        >;
        async fn set_as(
            &self,
            request: tonic::Request<super::SetAsRequest>,
        ) -> std::result::Result<tonic::Response<()>, tonic::Status>;
        async fn set_router_id(
            &self,
            request: tonic::Request<super::SetRouterIdRequest>,
        ) -> std::result::Result<tonic::Response<()>, tonic::Status>;
        async fn add_peer(
            &self,
            request: tonic::Request<super::AddPeerRequest>,
        ) -> std::result::Result<tonic::Response<()>, tonic::Status>;
        async fn delete_peer(
            &self,
            request: tonic::Request<super::DeletePeerRequest>,
        ) -> std::result::Result<tonic::Response<()>, tonic::Status>;
        async fn add_path(
            &self,
            request: tonic::Request<super::AddPathRequest>,
        ) -> std::result::Result<tonic::Response<super::AddPathResponse>, tonic::Status>;
        async fn delete_path(
            &self,
            request: tonic::Request<super::DeletePathRequest>,
        ) -> std::result::Result<
            tonic::Response<super::DeletePathResponse>,
            tonic::Status,
        >;
    }
    #[derive(Debug)]
    pub struct BgpApiServer<T: BgpApi> {
        inner: _Inner<T>,
        accept_compression_encodings: EnabledCompressionEncodings,
        send_compression_encodings: EnabledCompressionEncodings,
        max_decoding_message_size: Option<usize>,
        max_encoding_message_size: Option<usize>,
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
                max_decoding_message_size: None,
                max_encoding_message_size: None,
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
        /// Limits the maximum size of a decoded message.
        ///
        /// Default: `4MB`
        #[must_use]
        pub fn max_decoding_message_size(mut self, limit: usize) -> Self {
            self.max_decoding_message_size = Some(limit);
            self
        }
        /// Limits the maximum size of an encoded message.
        ///
        /// Default: `usize::MAX`
        #[must_use]
        pub fn max_encoding_message_size(mut self, limit: usize) -> Self {
            self.max_encoding_message_size = Some(limit);
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
        ) -> Poll<std::result::Result<(), Self::Error>> {
            Poll::Ready(Ok(()))
        }
        fn call(&mut self, req: http::Request<B>) -> Self::Future {
            let inner = self.inner.clone();
            match req.uri().path() {
                "/sart.v1.BgpApi/Health" => {
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
                            let inner = Arc::clone(&self.0);
                            let fut = async move {
                                <T as BgpApi>::health(&inner, request).await
                            };
                            Box::pin(fut)
                        }
                    }
                    let accept_compression_encodings = self.accept_compression_encodings;
                    let send_compression_encodings = self.send_compression_encodings;
                    let max_decoding_message_size = self.max_decoding_message_size;
                    let max_encoding_message_size = self.max_encoding_message_size;
                    let inner = self.inner.clone();
                    let fut = async move {
                        let inner = inner.0;
                        let method = HealthSvc(inner);
                        let codec = tonic::codec::ProstCodec::default();
                        let mut grpc = tonic::server::Grpc::new(codec)
                            .apply_compression_config(
                                accept_compression_encodings,
                                send_compression_encodings,
                            )
                            .apply_max_message_size_config(
                                max_decoding_message_size,
                                max_encoding_message_size,
                            );
                        let res = grpc.unary(method, req).await;
                        Ok(res)
                    };
                    Box::pin(fut)
                }
                "/sart.v1.BgpApi/GetBgpInfo" => {
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
                            let inner = Arc::clone(&self.0);
                            let fut = async move {
                                <T as BgpApi>::get_bgp_info(&inner, request).await
                            };
                            Box::pin(fut)
                        }
                    }
                    let accept_compression_encodings = self.accept_compression_encodings;
                    let send_compression_encodings = self.send_compression_encodings;
                    let max_decoding_message_size = self.max_decoding_message_size;
                    let max_encoding_message_size = self.max_encoding_message_size;
                    let inner = self.inner.clone();
                    let fut = async move {
                        let inner = inner.0;
                        let method = GetBgpInfoSvc(inner);
                        let codec = tonic::codec::ProstCodec::default();
                        let mut grpc = tonic::server::Grpc::new(codec)
                            .apply_compression_config(
                                accept_compression_encodings,
                                send_compression_encodings,
                            )
                            .apply_max_message_size_config(
                                max_decoding_message_size,
                                max_encoding_message_size,
                            );
                        let res = grpc.unary(method, req).await;
                        Ok(res)
                    };
                    Box::pin(fut)
                }
                "/sart.v1.BgpApi/GetNeighbor" => {
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
                            let inner = Arc::clone(&self.0);
                            let fut = async move {
                                <T as BgpApi>::get_neighbor(&inner, request).await
                            };
                            Box::pin(fut)
                        }
                    }
                    let accept_compression_encodings = self.accept_compression_encodings;
                    let send_compression_encodings = self.send_compression_encodings;
                    let max_decoding_message_size = self.max_decoding_message_size;
                    let max_encoding_message_size = self.max_encoding_message_size;
                    let inner = self.inner.clone();
                    let fut = async move {
                        let inner = inner.0;
                        let method = GetNeighborSvc(inner);
                        let codec = tonic::codec::ProstCodec::default();
                        let mut grpc = tonic::server::Grpc::new(codec)
                            .apply_compression_config(
                                accept_compression_encodings,
                                send_compression_encodings,
                            )
                            .apply_max_message_size_config(
                                max_decoding_message_size,
                                max_encoding_message_size,
                            );
                        let res = grpc.unary(method, req).await;
                        Ok(res)
                    };
                    Box::pin(fut)
                }
                "/sart.v1.BgpApi/ListNeighbor" => {
                    #[allow(non_camel_case_types)]
                    struct ListNeighborSvc<T: BgpApi>(pub Arc<T>);
                    impl<
                        T: BgpApi,
                    > tonic::server::UnaryService<super::ListNeighborRequest>
                    for ListNeighborSvc<T> {
                        type Response = super::ListNeighborResponse;
                        type Future = BoxFuture<
                            tonic::Response<Self::Response>,
                            tonic::Status,
                        >;
                        fn call(
                            &mut self,
                            request: tonic::Request<super::ListNeighborRequest>,
                        ) -> Self::Future {
                            let inner = Arc::clone(&self.0);
                            let fut = async move {
                                <T as BgpApi>::list_neighbor(&inner, request).await
                            };
                            Box::pin(fut)
                        }
                    }
                    let accept_compression_encodings = self.accept_compression_encodings;
                    let send_compression_encodings = self.send_compression_encodings;
                    let max_decoding_message_size = self.max_decoding_message_size;
                    let max_encoding_message_size = self.max_encoding_message_size;
                    let inner = self.inner.clone();
                    let fut = async move {
                        let inner = inner.0;
                        let method = ListNeighborSvc(inner);
                        let codec = tonic::codec::ProstCodec::default();
                        let mut grpc = tonic::server::Grpc::new(codec)
                            .apply_compression_config(
                                accept_compression_encodings,
                                send_compression_encodings,
                            )
                            .apply_max_message_size_config(
                                max_decoding_message_size,
                                max_encoding_message_size,
                            );
                        let res = grpc.unary(method, req).await;
                        Ok(res)
                    };
                    Box::pin(fut)
                }
                "/sart.v1.BgpApi/GetPath" => {
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
                            let inner = Arc::clone(&self.0);
                            let fut = async move {
                                <T as BgpApi>::get_path(&inner, request).await
                            };
                            Box::pin(fut)
                        }
                    }
                    let accept_compression_encodings = self.accept_compression_encodings;
                    let send_compression_encodings = self.send_compression_encodings;
                    let max_decoding_message_size = self.max_decoding_message_size;
                    let max_encoding_message_size = self.max_encoding_message_size;
                    let inner = self.inner.clone();
                    let fut = async move {
                        let inner = inner.0;
                        let method = GetPathSvc(inner);
                        let codec = tonic::codec::ProstCodec::default();
                        let mut grpc = tonic::server::Grpc::new(codec)
                            .apply_compression_config(
                                accept_compression_encodings,
                                send_compression_encodings,
                            )
                            .apply_max_message_size_config(
                                max_decoding_message_size,
                                max_encoding_message_size,
                            );
                        let res = grpc.unary(method, req).await;
                        Ok(res)
                    };
                    Box::pin(fut)
                }
                "/sart.v1.BgpApi/GetNeighborPath" => {
                    #[allow(non_camel_case_types)]
                    struct GetNeighborPathSvc<T: BgpApi>(pub Arc<T>);
                    impl<
                        T: BgpApi,
                    > tonic::server::UnaryService<super::GetNeighborPathRequest>
                    for GetNeighborPathSvc<T> {
                        type Response = super::GetNeighborPathResponse;
                        type Future = BoxFuture<
                            tonic::Response<Self::Response>,
                            tonic::Status,
                        >;
                        fn call(
                            &mut self,
                            request: tonic::Request<super::GetNeighborPathRequest>,
                        ) -> Self::Future {
                            let inner = Arc::clone(&self.0);
                            let fut = async move {
                                <T as BgpApi>::get_neighbor_path(&inner, request).await
                            };
                            Box::pin(fut)
                        }
                    }
                    let accept_compression_encodings = self.accept_compression_encodings;
                    let send_compression_encodings = self.send_compression_encodings;
                    let max_decoding_message_size = self.max_decoding_message_size;
                    let max_encoding_message_size = self.max_encoding_message_size;
                    let inner = self.inner.clone();
                    let fut = async move {
                        let inner = inner.0;
                        let method = GetNeighborPathSvc(inner);
                        let codec = tonic::codec::ProstCodec::default();
                        let mut grpc = tonic::server::Grpc::new(codec)
                            .apply_compression_config(
                                accept_compression_encodings,
                                send_compression_encodings,
                            )
                            .apply_max_message_size_config(
                                max_decoding_message_size,
                                max_encoding_message_size,
                            );
                        let res = grpc.unary(method, req).await;
                        Ok(res)
                    };
                    Box::pin(fut)
                }
                "/sart.v1.BgpApi/GetPathByPrefix" => {
                    #[allow(non_camel_case_types)]
                    struct GetPathByPrefixSvc<T: BgpApi>(pub Arc<T>);
                    impl<
                        T: BgpApi,
                    > tonic::server::UnaryService<super::GetPathByPrefixRequest>
                    for GetPathByPrefixSvc<T> {
                        type Response = super::GetPathByPrefixResponse;
                        type Future = BoxFuture<
                            tonic::Response<Self::Response>,
                            tonic::Status,
                        >;
                        fn call(
                            &mut self,
                            request: tonic::Request<super::GetPathByPrefixRequest>,
                        ) -> Self::Future {
                            let inner = Arc::clone(&self.0);
                            let fut = async move {
                                <T as BgpApi>::get_path_by_prefix(&inner, request).await
                            };
                            Box::pin(fut)
                        }
                    }
                    let accept_compression_encodings = self.accept_compression_encodings;
                    let send_compression_encodings = self.send_compression_encodings;
                    let max_decoding_message_size = self.max_decoding_message_size;
                    let max_encoding_message_size = self.max_encoding_message_size;
                    let inner = self.inner.clone();
                    let fut = async move {
                        let inner = inner.0;
                        let method = GetPathByPrefixSvc(inner);
                        let codec = tonic::codec::ProstCodec::default();
                        let mut grpc = tonic::server::Grpc::new(codec)
                            .apply_compression_config(
                                accept_compression_encodings,
                                send_compression_encodings,
                            )
                            .apply_max_message_size_config(
                                max_decoding_message_size,
                                max_encoding_message_size,
                            );
                        let res = grpc.unary(method, req).await;
                        Ok(res)
                    };
                    Box::pin(fut)
                }
                "/sart.v1.BgpApi/SetAS" => {
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
                            let inner = Arc::clone(&self.0);
                            let fut = async move {
                                <T as BgpApi>::set_as(&inner, request).await
                            };
                            Box::pin(fut)
                        }
                    }
                    let accept_compression_encodings = self.accept_compression_encodings;
                    let send_compression_encodings = self.send_compression_encodings;
                    let max_decoding_message_size = self.max_decoding_message_size;
                    let max_encoding_message_size = self.max_encoding_message_size;
                    let inner = self.inner.clone();
                    let fut = async move {
                        let inner = inner.0;
                        let method = SetASSvc(inner);
                        let codec = tonic::codec::ProstCodec::default();
                        let mut grpc = tonic::server::Grpc::new(codec)
                            .apply_compression_config(
                                accept_compression_encodings,
                                send_compression_encodings,
                            )
                            .apply_max_message_size_config(
                                max_decoding_message_size,
                                max_encoding_message_size,
                            );
                        let res = grpc.unary(method, req).await;
                        Ok(res)
                    };
                    Box::pin(fut)
                }
                "/sart.v1.BgpApi/SetRouterId" => {
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
                            let inner = Arc::clone(&self.0);
                            let fut = async move {
                                <T as BgpApi>::set_router_id(&inner, request).await
                            };
                            Box::pin(fut)
                        }
                    }
                    let accept_compression_encodings = self.accept_compression_encodings;
                    let send_compression_encodings = self.send_compression_encodings;
                    let max_decoding_message_size = self.max_decoding_message_size;
                    let max_encoding_message_size = self.max_encoding_message_size;
                    let inner = self.inner.clone();
                    let fut = async move {
                        let inner = inner.0;
                        let method = SetRouterIdSvc(inner);
                        let codec = tonic::codec::ProstCodec::default();
                        let mut grpc = tonic::server::Grpc::new(codec)
                            .apply_compression_config(
                                accept_compression_encodings,
                                send_compression_encodings,
                            )
                            .apply_max_message_size_config(
                                max_decoding_message_size,
                                max_encoding_message_size,
                            );
                        let res = grpc.unary(method, req).await;
                        Ok(res)
                    };
                    Box::pin(fut)
                }
                "/sart.v1.BgpApi/AddPeer" => {
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
                            let inner = Arc::clone(&self.0);
                            let fut = async move {
                                <T as BgpApi>::add_peer(&inner, request).await
                            };
                            Box::pin(fut)
                        }
                    }
                    let accept_compression_encodings = self.accept_compression_encodings;
                    let send_compression_encodings = self.send_compression_encodings;
                    let max_decoding_message_size = self.max_decoding_message_size;
                    let max_encoding_message_size = self.max_encoding_message_size;
                    let inner = self.inner.clone();
                    let fut = async move {
                        let inner = inner.0;
                        let method = AddPeerSvc(inner);
                        let codec = tonic::codec::ProstCodec::default();
                        let mut grpc = tonic::server::Grpc::new(codec)
                            .apply_compression_config(
                                accept_compression_encodings,
                                send_compression_encodings,
                            )
                            .apply_max_message_size_config(
                                max_decoding_message_size,
                                max_encoding_message_size,
                            );
                        let res = grpc.unary(method, req).await;
                        Ok(res)
                    };
                    Box::pin(fut)
                }
                "/sart.v1.BgpApi/DeletePeer" => {
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
                            let inner = Arc::clone(&self.0);
                            let fut = async move {
                                <T as BgpApi>::delete_peer(&inner, request).await
                            };
                            Box::pin(fut)
                        }
                    }
                    let accept_compression_encodings = self.accept_compression_encodings;
                    let send_compression_encodings = self.send_compression_encodings;
                    let max_decoding_message_size = self.max_decoding_message_size;
                    let max_encoding_message_size = self.max_encoding_message_size;
                    let inner = self.inner.clone();
                    let fut = async move {
                        let inner = inner.0;
                        let method = DeletePeerSvc(inner);
                        let codec = tonic::codec::ProstCodec::default();
                        let mut grpc = tonic::server::Grpc::new(codec)
                            .apply_compression_config(
                                accept_compression_encodings,
                                send_compression_encodings,
                            )
                            .apply_max_message_size_config(
                                max_decoding_message_size,
                                max_encoding_message_size,
                            );
                        let res = grpc.unary(method, req).await;
                        Ok(res)
                    };
                    Box::pin(fut)
                }
                "/sart.v1.BgpApi/AddPath" => {
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
                            let inner = Arc::clone(&self.0);
                            let fut = async move {
                                <T as BgpApi>::add_path(&inner, request).await
                            };
                            Box::pin(fut)
                        }
                    }
                    let accept_compression_encodings = self.accept_compression_encodings;
                    let send_compression_encodings = self.send_compression_encodings;
                    let max_decoding_message_size = self.max_decoding_message_size;
                    let max_encoding_message_size = self.max_encoding_message_size;
                    let inner = self.inner.clone();
                    let fut = async move {
                        let inner = inner.0;
                        let method = AddPathSvc(inner);
                        let codec = tonic::codec::ProstCodec::default();
                        let mut grpc = tonic::server::Grpc::new(codec)
                            .apply_compression_config(
                                accept_compression_encodings,
                                send_compression_encodings,
                            )
                            .apply_max_message_size_config(
                                max_decoding_message_size,
                                max_encoding_message_size,
                            );
                        let res = grpc.unary(method, req).await;
                        Ok(res)
                    };
                    Box::pin(fut)
                }
                "/sart.v1.BgpApi/DeletePath" => {
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
                            let inner = Arc::clone(&self.0);
                            let fut = async move {
                                <T as BgpApi>::delete_path(&inner, request).await
                            };
                            Box::pin(fut)
                        }
                    }
                    let accept_compression_encodings = self.accept_compression_encodings;
                    let send_compression_encodings = self.send_compression_encodings;
                    let max_decoding_message_size = self.max_decoding_message_size;
                    let max_encoding_message_size = self.max_encoding_message_size;
                    let inner = self.inner.clone();
                    let fut = async move {
                        let inner = inner.0;
                        let method = DeletePathSvc(inner);
                        let codec = tonic::codec::ProstCodec::default();
                        let mut grpc = tonic::server::Grpc::new(codec)
                            .apply_compression_config(
                                accept_compression_encodings,
                                send_compression_encodings,
                            )
                            .apply_max_message_size_config(
                                max_decoding_message_size,
                                max_encoding_message_size,
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
                max_decoding_message_size: self.max_decoding_message_size,
                max_encoding_message_size: self.max_encoding_message_size,
            }
        }
    }
    impl<T: BgpApi> Clone for _Inner<T> {
        fn clone(&self) -> Self {
            Self(Arc::clone(&self.0))
        }
    }
    impl<T: std::fmt::Debug> std::fmt::Debug for _Inner<T> {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "{:?}", self.0)
        }
    }
    impl<T: BgpApi> tonic::server::NamedService for BgpApiServer<T> {
        const NAME: &'static str = "sart.v1.BgpApi";
    }
}
/// Generated server implementations.
pub mod bgp_exporter_api_server {
    #![allow(unused_variables, dead_code, missing_docs, clippy::let_unit_value)]
    use tonic::codegen::*;
    /// Generated trait containing gRPC methods that should be implemented for use with BgpExporterApiServer.
    #[async_trait]
    pub trait BgpExporterApi: Send + Sync + 'static {
        async fn export_peer(
            &self,
            request: tonic::Request<super::ExportPeerRequest>,
        ) -> std::result::Result<tonic::Response<()>, tonic::Status>;
        async fn export_peer_state(
            &self,
            request: tonic::Request<super::ExportPeerStateRequest>,
        ) -> std::result::Result<tonic::Response<()>, tonic::Status>;
    }
    #[derive(Debug)]
    pub struct BgpExporterApiServer<T: BgpExporterApi> {
        inner: _Inner<T>,
        accept_compression_encodings: EnabledCompressionEncodings,
        send_compression_encodings: EnabledCompressionEncodings,
        max_decoding_message_size: Option<usize>,
        max_encoding_message_size: Option<usize>,
    }
    struct _Inner<T>(Arc<T>);
    impl<T: BgpExporterApi> BgpExporterApiServer<T> {
        pub fn new(inner: T) -> Self {
            Self::from_arc(Arc::new(inner))
        }
        pub fn from_arc(inner: Arc<T>) -> Self {
            let inner = _Inner(inner);
            Self {
                inner,
                accept_compression_encodings: Default::default(),
                send_compression_encodings: Default::default(),
                max_decoding_message_size: None,
                max_encoding_message_size: None,
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
        /// Limits the maximum size of a decoded message.
        ///
        /// Default: `4MB`
        #[must_use]
        pub fn max_decoding_message_size(mut self, limit: usize) -> Self {
            self.max_decoding_message_size = Some(limit);
            self
        }
        /// Limits the maximum size of an encoded message.
        ///
        /// Default: `usize::MAX`
        #[must_use]
        pub fn max_encoding_message_size(mut self, limit: usize) -> Self {
            self.max_encoding_message_size = Some(limit);
            self
        }
    }
    impl<T, B> tonic::codegen::Service<http::Request<B>> for BgpExporterApiServer<T>
    where
        T: BgpExporterApi,
        B: Body + Send + 'static,
        B::Error: Into<StdError> + Send + 'static,
    {
        type Response = http::Response<tonic::body::BoxBody>;
        type Error = std::convert::Infallible;
        type Future = BoxFuture<Self::Response, Self::Error>;
        fn poll_ready(
            &mut self,
            _cx: &mut Context<'_>,
        ) -> Poll<std::result::Result<(), Self::Error>> {
            Poll::Ready(Ok(()))
        }
        fn call(&mut self, req: http::Request<B>) -> Self::Future {
            let inner = self.inner.clone();
            match req.uri().path() {
                "/sart.v1.BgpExporterApi/ExportPeer" => {
                    #[allow(non_camel_case_types)]
                    struct ExportPeerSvc<T: BgpExporterApi>(pub Arc<T>);
                    impl<
                        T: BgpExporterApi,
                    > tonic::server::UnaryService<super::ExportPeerRequest>
                    for ExportPeerSvc<T> {
                        type Response = ();
                        type Future = BoxFuture<
                            tonic::Response<Self::Response>,
                            tonic::Status,
                        >;
                        fn call(
                            &mut self,
                            request: tonic::Request<super::ExportPeerRequest>,
                        ) -> Self::Future {
                            let inner = Arc::clone(&self.0);
                            let fut = async move {
                                <T as BgpExporterApi>::export_peer(&inner, request).await
                            };
                            Box::pin(fut)
                        }
                    }
                    let accept_compression_encodings = self.accept_compression_encodings;
                    let send_compression_encodings = self.send_compression_encodings;
                    let max_decoding_message_size = self.max_decoding_message_size;
                    let max_encoding_message_size = self.max_encoding_message_size;
                    let inner = self.inner.clone();
                    let fut = async move {
                        let inner = inner.0;
                        let method = ExportPeerSvc(inner);
                        let codec = tonic::codec::ProstCodec::default();
                        let mut grpc = tonic::server::Grpc::new(codec)
                            .apply_compression_config(
                                accept_compression_encodings,
                                send_compression_encodings,
                            )
                            .apply_max_message_size_config(
                                max_decoding_message_size,
                                max_encoding_message_size,
                            );
                        let res = grpc.unary(method, req).await;
                        Ok(res)
                    };
                    Box::pin(fut)
                }
                "/sart.v1.BgpExporterApi/ExportPeerState" => {
                    #[allow(non_camel_case_types)]
                    struct ExportPeerStateSvc<T: BgpExporterApi>(pub Arc<T>);
                    impl<
                        T: BgpExporterApi,
                    > tonic::server::UnaryService<super::ExportPeerStateRequest>
                    for ExportPeerStateSvc<T> {
                        type Response = ();
                        type Future = BoxFuture<
                            tonic::Response<Self::Response>,
                            tonic::Status,
                        >;
                        fn call(
                            &mut self,
                            request: tonic::Request<super::ExportPeerStateRequest>,
                        ) -> Self::Future {
                            let inner = Arc::clone(&self.0);
                            let fut = async move {
                                <T as BgpExporterApi>::export_peer_state(&inner, request)
                                    .await
                            };
                            Box::pin(fut)
                        }
                    }
                    let accept_compression_encodings = self.accept_compression_encodings;
                    let send_compression_encodings = self.send_compression_encodings;
                    let max_decoding_message_size = self.max_decoding_message_size;
                    let max_encoding_message_size = self.max_encoding_message_size;
                    let inner = self.inner.clone();
                    let fut = async move {
                        let inner = inner.0;
                        let method = ExportPeerStateSvc(inner);
                        let codec = tonic::codec::ProstCodec::default();
                        let mut grpc = tonic::server::Grpc::new(codec)
                            .apply_compression_config(
                                accept_compression_encodings,
                                send_compression_encodings,
                            )
                            .apply_max_message_size_config(
                                max_decoding_message_size,
                                max_encoding_message_size,
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
    impl<T: BgpExporterApi> Clone for BgpExporterApiServer<T> {
        fn clone(&self) -> Self {
            let inner = self.inner.clone();
            Self {
                inner,
                accept_compression_encodings: self.accept_compression_encodings,
                send_compression_encodings: self.send_compression_encodings,
                max_decoding_message_size: self.max_decoding_message_size,
                max_encoding_message_size: self.max_encoding_message_size,
            }
        }
    }
    impl<T: BgpExporterApi> Clone for _Inner<T> {
        fn clone(&self) -> Self {
            Self(Arc::clone(&self.0))
        }
    }
    impl<T: std::fmt::Debug> std::fmt::Debug for _Inner<T> {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "{:?}", self.0)
        }
    }
    impl<T: BgpExporterApi> tonic::server::NamedService for BgpExporterApiServer<T> {
        const NAME: &'static str = "sart.v1.BgpExporterApi";
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
    #[prost(message, optional, tag = "3")]
    pub route: ::core::option::Option<Route>,
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
        /// Creates an enum from field names used in the ProtoBuf definition.
        pub fn from_str_name(value: &str) -> ::core::option::Option<Self> {
            match value {
                "EMPTY" => Some(Self::Empty),
                "DEAD" => Some(Self::Dead),
                "PERVASIVE" => Some(Self::Pervasive),
                "ONLINK" => Some(Self::Onlink),
                "OFFLOAD" => Some(Self::Offload),
                "LINKDOWN" => Some(Self::Linkdown),
                "UNRESOLVED" => Some(Self::Unresolved),
                _ => None,
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
    /// Creates an enum from field names used in the ProtoBuf definition.
    pub fn from_str_name(value: &str) -> ::core::option::Option<Self> {
        match value {
            "Unkown" => Some(Self::Unkown),
            "V4" => Some(Self::V4),
            "V6" => Some(Self::V6),
            _ => None,
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
    /// Creates an enum from field names used in the ProtoBuf definition.
    pub fn from_str_name(value: &str) -> ::core::option::Option<Self> {
        match value {
            "ADConnected" => Some(Self::AdConnected),
            "ADStatic" => Some(Self::AdStatic),
            "ADEBGP" => Some(Self::Adebgp),
            "ADOSPF" => Some(Self::Adospf),
            "ADRIP" => Some(Self::Adrip),
            "ADIBGP" => Some(Self::Adibgp),
            _ => None,
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
    /// Creates an enum from field names used in the ProtoBuf definition.
    pub fn from_str_name(value: &str) -> ::core::option::Option<Self> {
        match value {
            "Unspec" => Some(Self::Unspec),
            "Redirect" => Some(Self::Redirect),
            "Kernel" => Some(Self::Kernel),
            "Boot" => Some(Self::Boot),
            "Static" => Some(Self::Static),
            "Bgp" => Some(Self::Bgp),
            "IsIs" => Some(Self::IsIs),
            "Ospf" => Some(Self::Ospf),
            "Rip" => Some(Self::Rip),
            _ => None,
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
    /// Creates an enum from field names used in the ProtoBuf definition.
    pub fn from_str_name(value: &str) -> ::core::option::Option<Self> {
        match value {
            "UnspecType" => Some(Self::UnspecType),
            "Unicast" => Some(Self::Unicast),
            "Local" => Some(Self::Local),
            "Broadcast" => Some(Self::Broadcast),
            "Anycast" => Some(Self::Anycast),
            "Multicast" => Some(Self::Multicast),
            "Blackhole" => Some(Self::Blackhole),
            "Unreachable" => Some(Self::Unreachable),
            "Prohibit" => Some(Self::Prohibit),
            "Throw" => Some(Self::Throw),
            "Nat" => Some(Self::Nat),
            _ => None,
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
    /// Creates an enum from field names used in the ProtoBuf definition.
    pub fn from_str_name(value: &str) -> ::core::option::Option<Self> {
        match value {
            "Universe" => Some(Self::Universe),
            "Site" => Some(Self::Site),
            "Link" => Some(Self::Link),
            "Host" => Some(Self::Host),
            "Nowhere" => Some(Self::Nowhere),
            _ => None,
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
            D: TryInto<tonic::transport::Endpoint>,
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
        /// Limits the maximum size of a decoded message.
        ///
        /// Default: `4MB`
        #[must_use]
        pub fn max_decoding_message_size(mut self, limit: usize) -> Self {
            self.inner = self.inner.max_decoding_message_size(limit);
            self
        }
        /// Limits the maximum size of an encoded message.
        ///
        /// Default: `usize::MAX`
        #[must_use]
        pub fn max_encoding_message_size(mut self, limit: usize) -> Self {
            self.inner = self.inner.max_encoding_message_size(limit);
            self
        }
        pub async fn get_route(
            &mut self,
            request: impl tonic::IntoRequest<super::GetRouteRequest>,
        ) -> std::result::Result<
            tonic::Response<super::GetRouteResponse>,
            tonic::Status,
        > {
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
            let path = http::uri::PathAndQuery::from_static("/sart.v1.FibApi/GetRoute");
            let mut req = request.into_request();
            req.extensions_mut().insert(GrpcMethod::new("sart.v1.FibApi", "GetRoute"));
            self.inner.unary(req, path, codec).await
        }
        pub async fn list_routes(
            &mut self,
            request: impl tonic::IntoRequest<super::ListRoutesRequest>,
        ) -> std::result::Result<
            tonic::Response<super::ListRoutesResponse>,
            tonic::Status,
        > {
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
                "/sart.v1.FibApi/ListRoutes",
            );
            let mut req = request.into_request();
            req.extensions_mut().insert(GrpcMethod::new("sart.v1.FibApi", "ListRoutes"));
            self.inner.unary(req, path, codec).await
        }
        pub async fn add_route(
            &mut self,
            request: impl tonic::IntoRequest<super::AddRouteRequest>,
        ) -> std::result::Result<tonic::Response<()>, tonic::Status> {
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
            let path = http::uri::PathAndQuery::from_static("/sart.v1.FibApi/AddRoute");
            let mut req = request.into_request();
            req.extensions_mut().insert(GrpcMethod::new("sart.v1.FibApi", "AddRoute"));
            self.inner.unary(req, path, codec).await
        }
        pub async fn delete_route(
            &mut self,
            request: impl tonic::IntoRequest<super::DeleteRouteRequest>,
        ) -> std::result::Result<tonic::Response<()>, tonic::Status> {
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
                "/sart.v1.FibApi/DeleteRoute",
            );
            let mut req = request.into_request();
            req.extensions_mut()
                .insert(GrpcMethod::new("sart.v1.FibApi", "DeleteRoute"));
            self.inner.unary(req, path, codec).await
        }
        pub async fn add_multi_path_route(
            &mut self,
            request: impl tonic::IntoRequest<super::AddMultiPathRouteRequest>,
        ) -> std::result::Result<tonic::Response<()>, tonic::Status> {
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
                "/sart.v1.FibApi/AddMultiPathRoute",
            );
            let mut req = request.into_request();
            req.extensions_mut()
                .insert(GrpcMethod::new("sart.v1.FibApi", "AddMultiPathRoute"));
            self.inner.unary(req, path, codec).await
        }
        pub async fn delete_multi_path_route(
            &mut self,
            request: impl tonic::IntoRequest<super::DeleteMultiPathRouteRequest>,
        ) -> std::result::Result<tonic::Response<()>, tonic::Status> {
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
                "/sart.v1.FibApi/DeleteMultiPathRoute",
            );
            let mut req = request.into_request();
            req.extensions_mut()
                .insert(GrpcMethod::new("sart.v1.FibApi", "DeleteMultiPathRoute"));
            self.inner.unary(req, path, codec).await
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
        ) -> std::result::Result<
            tonic::Response<super::GetRouteResponse>,
            tonic::Status,
        >;
        async fn list_routes(
            &self,
            request: tonic::Request<super::ListRoutesRequest>,
        ) -> std::result::Result<
            tonic::Response<super::ListRoutesResponse>,
            tonic::Status,
        >;
        async fn add_route(
            &self,
            request: tonic::Request<super::AddRouteRequest>,
        ) -> std::result::Result<tonic::Response<()>, tonic::Status>;
        async fn delete_route(
            &self,
            request: tonic::Request<super::DeleteRouteRequest>,
        ) -> std::result::Result<tonic::Response<()>, tonic::Status>;
        async fn add_multi_path_route(
            &self,
            request: tonic::Request<super::AddMultiPathRouteRequest>,
        ) -> std::result::Result<tonic::Response<()>, tonic::Status>;
        async fn delete_multi_path_route(
            &self,
            request: tonic::Request<super::DeleteMultiPathRouteRequest>,
        ) -> std::result::Result<tonic::Response<()>, tonic::Status>;
    }
    #[derive(Debug)]
    pub struct FibApiServer<T: FibApi> {
        inner: _Inner<T>,
        accept_compression_encodings: EnabledCompressionEncodings,
        send_compression_encodings: EnabledCompressionEncodings,
        max_decoding_message_size: Option<usize>,
        max_encoding_message_size: Option<usize>,
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
                max_decoding_message_size: None,
                max_encoding_message_size: None,
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
        /// Limits the maximum size of a decoded message.
        ///
        /// Default: `4MB`
        #[must_use]
        pub fn max_decoding_message_size(mut self, limit: usize) -> Self {
            self.max_decoding_message_size = Some(limit);
            self
        }
        /// Limits the maximum size of an encoded message.
        ///
        /// Default: `usize::MAX`
        #[must_use]
        pub fn max_encoding_message_size(mut self, limit: usize) -> Self {
            self.max_encoding_message_size = Some(limit);
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
        ) -> Poll<std::result::Result<(), Self::Error>> {
            Poll::Ready(Ok(()))
        }
        fn call(&mut self, req: http::Request<B>) -> Self::Future {
            let inner = self.inner.clone();
            match req.uri().path() {
                "/sart.v1.FibApi/GetRoute" => {
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
                            let inner = Arc::clone(&self.0);
                            let fut = async move {
                                <T as FibApi>::get_route(&inner, request).await
                            };
                            Box::pin(fut)
                        }
                    }
                    let accept_compression_encodings = self.accept_compression_encodings;
                    let send_compression_encodings = self.send_compression_encodings;
                    let max_decoding_message_size = self.max_decoding_message_size;
                    let max_encoding_message_size = self.max_encoding_message_size;
                    let inner = self.inner.clone();
                    let fut = async move {
                        let inner = inner.0;
                        let method = GetRouteSvc(inner);
                        let codec = tonic::codec::ProstCodec::default();
                        let mut grpc = tonic::server::Grpc::new(codec)
                            .apply_compression_config(
                                accept_compression_encodings,
                                send_compression_encodings,
                            )
                            .apply_max_message_size_config(
                                max_decoding_message_size,
                                max_encoding_message_size,
                            );
                        let res = grpc.unary(method, req).await;
                        Ok(res)
                    };
                    Box::pin(fut)
                }
                "/sart.v1.FibApi/ListRoutes" => {
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
                            let inner = Arc::clone(&self.0);
                            let fut = async move {
                                <T as FibApi>::list_routes(&inner, request).await
                            };
                            Box::pin(fut)
                        }
                    }
                    let accept_compression_encodings = self.accept_compression_encodings;
                    let send_compression_encodings = self.send_compression_encodings;
                    let max_decoding_message_size = self.max_decoding_message_size;
                    let max_encoding_message_size = self.max_encoding_message_size;
                    let inner = self.inner.clone();
                    let fut = async move {
                        let inner = inner.0;
                        let method = ListRoutesSvc(inner);
                        let codec = tonic::codec::ProstCodec::default();
                        let mut grpc = tonic::server::Grpc::new(codec)
                            .apply_compression_config(
                                accept_compression_encodings,
                                send_compression_encodings,
                            )
                            .apply_max_message_size_config(
                                max_decoding_message_size,
                                max_encoding_message_size,
                            );
                        let res = grpc.unary(method, req).await;
                        Ok(res)
                    };
                    Box::pin(fut)
                }
                "/sart.v1.FibApi/AddRoute" => {
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
                            let inner = Arc::clone(&self.0);
                            let fut = async move {
                                <T as FibApi>::add_route(&inner, request).await
                            };
                            Box::pin(fut)
                        }
                    }
                    let accept_compression_encodings = self.accept_compression_encodings;
                    let send_compression_encodings = self.send_compression_encodings;
                    let max_decoding_message_size = self.max_decoding_message_size;
                    let max_encoding_message_size = self.max_encoding_message_size;
                    let inner = self.inner.clone();
                    let fut = async move {
                        let inner = inner.0;
                        let method = AddRouteSvc(inner);
                        let codec = tonic::codec::ProstCodec::default();
                        let mut grpc = tonic::server::Grpc::new(codec)
                            .apply_compression_config(
                                accept_compression_encodings,
                                send_compression_encodings,
                            )
                            .apply_max_message_size_config(
                                max_decoding_message_size,
                                max_encoding_message_size,
                            );
                        let res = grpc.unary(method, req).await;
                        Ok(res)
                    };
                    Box::pin(fut)
                }
                "/sart.v1.FibApi/DeleteRoute" => {
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
                            let inner = Arc::clone(&self.0);
                            let fut = async move {
                                <T as FibApi>::delete_route(&inner, request).await
                            };
                            Box::pin(fut)
                        }
                    }
                    let accept_compression_encodings = self.accept_compression_encodings;
                    let send_compression_encodings = self.send_compression_encodings;
                    let max_decoding_message_size = self.max_decoding_message_size;
                    let max_encoding_message_size = self.max_encoding_message_size;
                    let inner = self.inner.clone();
                    let fut = async move {
                        let inner = inner.0;
                        let method = DeleteRouteSvc(inner);
                        let codec = tonic::codec::ProstCodec::default();
                        let mut grpc = tonic::server::Grpc::new(codec)
                            .apply_compression_config(
                                accept_compression_encodings,
                                send_compression_encodings,
                            )
                            .apply_max_message_size_config(
                                max_decoding_message_size,
                                max_encoding_message_size,
                            );
                        let res = grpc.unary(method, req).await;
                        Ok(res)
                    };
                    Box::pin(fut)
                }
                "/sart.v1.FibApi/AddMultiPathRoute" => {
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
                            let inner = Arc::clone(&self.0);
                            let fut = async move {
                                <T as FibApi>::add_multi_path_route(&inner, request).await
                            };
                            Box::pin(fut)
                        }
                    }
                    let accept_compression_encodings = self.accept_compression_encodings;
                    let send_compression_encodings = self.send_compression_encodings;
                    let max_decoding_message_size = self.max_decoding_message_size;
                    let max_encoding_message_size = self.max_encoding_message_size;
                    let inner = self.inner.clone();
                    let fut = async move {
                        let inner = inner.0;
                        let method = AddMultiPathRouteSvc(inner);
                        let codec = tonic::codec::ProstCodec::default();
                        let mut grpc = tonic::server::Grpc::new(codec)
                            .apply_compression_config(
                                accept_compression_encodings,
                                send_compression_encodings,
                            )
                            .apply_max_message_size_config(
                                max_decoding_message_size,
                                max_encoding_message_size,
                            );
                        let res = grpc.unary(method, req).await;
                        Ok(res)
                    };
                    Box::pin(fut)
                }
                "/sart.v1.FibApi/DeleteMultiPathRoute" => {
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
                            let inner = Arc::clone(&self.0);
                            let fut = async move {
                                <T as FibApi>::delete_multi_path_route(&inner, request)
                                    .await
                            };
                            Box::pin(fut)
                        }
                    }
                    let accept_compression_encodings = self.accept_compression_encodings;
                    let send_compression_encodings = self.send_compression_encodings;
                    let max_decoding_message_size = self.max_decoding_message_size;
                    let max_encoding_message_size = self.max_encoding_message_size;
                    let inner = self.inner.clone();
                    let fut = async move {
                        let inner = inner.0;
                        let method = DeleteMultiPathRouteSvc(inner);
                        let codec = tonic::codec::ProstCodec::default();
                        let mut grpc = tonic::server::Grpc::new(codec)
                            .apply_compression_config(
                                accept_compression_encodings,
                                send_compression_encodings,
                            )
                            .apply_max_message_size_config(
                                max_decoding_message_size,
                                max_encoding_message_size,
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
                max_decoding_message_size: self.max_decoding_message_size,
                max_encoding_message_size: self.max_encoding_message_size,
            }
        }
    }
    impl<T: FibApi> Clone for _Inner<T> {
        fn clone(&self) -> Self {
            Self(Arc::clone(&self.0))
        }
    }
    impl<T: std::fmt::Debug> std::fmt::Debug for _Inner<T> {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "{:?}", self.0)
        }
    }
    impl<T: FibApi> tonic::server::NamedService for FibApiServer<T> {
        const NAME: &'static str = "sart.v1.FibApi";
    }
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetChannelRequest {
    #[prost(string, tag = "1")]
    pub name: ::prost::alloc::string::String,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetChannelResponse {
    #[prost(message, optional, tag = "1")]
    pub channel: ::core::option::Option<Channel>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ListChannelRequest {}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ListChannelResponse {
    #[prost(message, repeated, tag = "1")]
    pub channels: ::prost::alloc::vec::Vec<Channel>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Channel {
    #[prost(string, tag = "1")]
    pub name: ::prost::alloc::string::String,
    #[prost(message, repeated, tag = "2")]
    pub subscribers: ::prost::alloc::vec::Vec<ChProtocol>,
    #[prost(message, repeated, tag = "3")]
    pub publishers: ::prost::alloc::vec::Vec<ChProtocol>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ChProtocol {
    #[prost(string, tag = "1")]
    pub r#type: ::prost::alloc::string::String,
    /// for bgp type
    #[prost(string, tag = "2")]
    pub endpoint: ::prost::alloc::string::String,
    /// for kernel type
    #[prost(int32, repeated, tag = "3")]
    pub tables: ::prost::alloc::vec::Vec<i32>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetRoutesRequest {
    #[prost(string, tag = "1")]
    pub channel: ::prost::alloc::string::String,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetRoutesResponse {
    #[prost(message, repeated, tag = "1")]
    pub routes: ::prost::alloc::vec::Vec<Route>,
}
/// Generated client implementations.
pub mod fib_manager_api_client {
    #![allow(unused_variables, dead_code, missing_docs, clippy::let_unit_value)]
    use tonic::codegen::*;
    use tonic::codegen::http::Uri;
    #[derive(Debug, Clone)]
    pub struct FibManagerApiClient<T> {
        inner: tonic::client::Grpc<T>,
    }
    impl FibManagerApiClient<tonic::transport::Channel> {
        /// Attempt to create a new client by connecting to a given endpoint.
        pub async fn connect<D>(dst: D) -> Result<Self, tonic::transport::Error>
        where
            D: TryInto<tonic::transport::Endpoint>,
            D::Error: Into<StdError>,
        {
            let conn = tonic::transport::Endpoint::new(dst)?.connect().await?;
            Ok(Self::new(conn))
        }
    }
    impl<T> FibManagerApiClient<T>
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
        ) -> FibManagerApiClient<InterceptedService<T, F>>
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
            FibManagerApiClient::new(InterceptedService::new(inner, interceptor))
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
        /// Limits the maximum size of a decoded message.
        ///
        /// Default: `4MB`
        #[must_use]
        pub fn max_decoding_message_size(mut self, limit: usize) -> Self {
            self.inner = self.inner.max_decoding_message_size(limit);
            self
        }
        /// Limits the maximum size of an encoded message.
        ///
        /// Default: `usize::MAX`
        #[must_use]
        pub fn max_encoding_message_size(mut self, limit: usize) -> Self {
            self.inner = self.inner.max_encoding_message_size(limit);
            self
        }
        pub async fn get_channel(
            &mut self,
            request: impl tonic::IntoRequest<super::GetChannelRequest>,
        ) -> std::result::Result<
            tonic::Response<super::GetChannelResponse>,
            tonic::Status,
        > {
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
                "/sart.v1.FibManagerApi/GetChannel",
            );
            let mut req = request.into_request();
            req.extensions_mut()
                .insert(GrpcMethod::new("sart.v1.FibManagerApi", "GetChannel"));
            self.inner.unary(req, path, codec).await
        }
        pub async fn list_channel(
            &mut self,
            request: impl tonic::IntoRequest<super::ListChannelRequest>,
        ) -> std::result::Result<
            tonic::Response<super::ListChannelResponse>,
            tonic::Status,
        > {
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
                "/sart.v1.FibManagerApi/ListChannel",
            );
            let mut req = request.into_request();
            req.extensions_mut()
                .insert(GrpcMethod::new("sart.v1.FibManagerApi", "ListChannel"));
            self.inner.unary(req, path, codec).await
        }
        pub async fn get_routes(
            &mut self,
            request: impl tonic::IntoRequest<super::GetRoutesRequest>,
        ) -> std::result::Result<
            tonic::Response<super::GetRoutesResponse>,
            tonic::Status,
        > {
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
                "/sart.v1.FibManagerApi/GetRoutes",
            );
            let mut req = request.into_request();
            req.extensions_mut()
                .insert(GrpcMethod::new("sart.v1.FibManagerApi", "GetRoutes"));
            self.inner.unary(req, path, codec).await
        }
    }
}
/// Generated server implementations.
pub mod fib_manager_api_server {
    #![allow(unused_variables, dead_code, missing_docs, clippy::let_unit_value)]
    use tonic::codegen::*;
    /// Generated trait containing gRPC methods that should be implemented for use with FibManagerApiServer.
    #[async_trait]
    pub trait FibManagerApi: Send + Sync + 'static {
        async fn get_channel(
            &self,
            request: tonic::Request<super::GetChannelRequest>,
        ) -> std::result::Result<
            tonic::Response<super::GetChannelResponse>,
            tonic::Status,
        >;
        async fn list_channel(
            &self,
            request: tonic::Request<super::ListChannelRequest>,
        ) -> std::result::Result<
            tonic::Response<super::ListChannelResponse>,
            tonic::Status,
        >;
        async fn get_routes(
            &self,
            request: tonic::Request<super::GetRoutesRequest>,
        ) -> std::result::Result<
            tonic::Response<super::GetRoutesResponse>,
            tonic::Status,
        >;
    }
    #[derive(Debug)]
    pub struct FibManagerApiServer<T: FibManagerApi> {
        inner: _Inner<T>,
        accept_compression_encodings: EnabledCompressionEncodings,
        send_compression_encodings: EnabledCompressionEncodings,
        max_decoding_message_size: Option<usize>,
        max_encoding_message_size: Option<usize>,
    }
    struct _Inner<T>(Arc<T>);
    impl<T: FibManagerApi> FibManagerApiServer<T> {
        pub fn new(inner: T) -> Self {
            Self::from_arc(Arc::new(inner))
        }
        pub fn from_arc(inner: Arc<T>) -> Self {
            let inner = _Inner(inner);
            Self {
                inner,
                accept_compression_encodings: Default::default(),
                send_compression_encodings: Default::default(),
                max_decoding_message_size: None,
                max_encoding_message_size: None,
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
        /// Limits the maximum size of a decoded message.
        ///
        /// Default: `4MB`
        #[must_use]
        pub fn max_decoding_message_size(mut self, limit: usize) -> Self {
            self.max_decoding_message_size = Some(limit);
            self
        }
        /// Limits the maximum size of an encoded message.
        ///
        /// Default: `usize::MAX`
        #[must_use]
        pub fn max_encoding_message_size(mut self, limit: usize) -> Self {
            self.max_encoding_message_size = Some(limit);
            self
        }
    }
    impl<T, B> tonic::codegen::Service<http::Request<B>> for FibManagerApiServer<T>
    where
        T: FibManagerApi,
        B: Body + Send + 'static,
        B::Error: Into<StdError> + Send + 'static,
    {
        type Response = http::Response<tonic::body::BoxBody>;
        type Error = std::convert::Infallible;
        type Future = BoxFuture<Self::Response, Self::Error>;
        fn poll_ready(
            &mut self,
            _cx: &mut Context<'_>,
        ) -> Poll<std::result::Result<(), Self::Error>> {
            Poll::Ready(Ok(()))
        }
        fn call(&mut self, req: http::Request<B>) -> Self::Future {
            let inner = self.inner.clone();
            match req.uri().path() {
                "/sart.v1.FibManagerApi/GetChannel" => {
                    #[allow(non_camel_case_types)]
                    struct GetChannelSvc<T: FibManagerApi>(pub Arc<T>);
                    impl<
                        T: FibManagerApi,
                    > tonic::server::UnaryService<super::GetChannelRequest>
                    for GetChannelSvc<T> {
                        type Response = super::GetChannelResponse;
                        type Future = BoxFuture<
                            tonic::Response<Self::Response>,
                            tonic::Status,
                        >;
                        fn call(
                            &mut self,
                            request: tonic::Request<super::GetChannelRequest>,
                        ) -> Self::Future {
                            let inner = Arc::clone(&self.0);
                            let fut = async move {
                                <T as FibManagerApi>::get_channel(&inner, request).await
                            };
                            Box::pin(fut)
                        }
                    }
                    let accept_compression_encodings = self.accept_compression_encodings;
                    let send_compression_encodings = self.send_compression_encodings;
                    let max_decoding_message_size = self.max_decoding_message_size;
                    let max_encoding_message_size = self.max_encoding_message_size;
                    let inner = self.inner.clone();
                    let fut = async move {
                        let inner = inner.0;
                        let method = GetChannelSvc(inner);
                        let codec = tonic::codec::ProstCodec::default();
                        let mut grpc = tonic::server::Grpc::new(codec)
                            .apply_compression_config(
                                accept_compression_encodings,
                                send_compression_encodings,
                            )
                            .apply_max_message_size_config(
                                max_decoding_message_size,
                                max_encoding_message_size,
                            );
                        let res = grpc.unary(method, req).await;
                        Ok(res)
                    };
                    Box::pin(fut)
                }
                "/sart.v1.FibManagerApi/ListChannel" => {
                    #[allow(non_camel_case_types)]
                    struct ListChannelSvc<T: FibManagerApi>(pub Arc<T>);
                    impl<
                        T: FibManagerApi,
                    > tonic::server::UnaryService<super::ListChannelRequest>
                    for ListChannelSvc<T> {
                        type Response = super::ListChannelResponse;
                        type Future = BoxFuture<
                            tonic::Response<Self::Response>,
                            tonic::Status,
                        >;
                        fn call(
                            &mut self,
                            request: tonic::Request<super::ListChannelRequest>,
                        ) -> Self::Future {
                            let inner = Arc::clone(&self.0);
                            let fut = async move {
                                <T as FibManagerApi>::list_channel(&inner, request).await
                            };
                            Box::pin(fut)
                        }
                    }
                    let accept_compression_encodings = self.accept_compression_encodings;
                    let send_compression_encodings = self.send_compression_encodings;
                    let max_decoding_message_size = self.max_decoding_message_size;
                    let max_encoding_message_size = self.max_encoding_message_size;
                    let inner = self.inner.clone();
                    let fut = async move {
                        let inner = inner.0;
                        let method = ListChannelSvc(inner);
                        let codec = tonic::codec::ProstCodec::default();
                        let mut grpc = tonic::server::Grpc::new(codec)
                            .apply_compression_config(
                                accept_compression_encodings,
                                send_compression_encodings,
                            )
                            .apply_max_message_size_config(
                                max_decoding_message_size,
                                max_encoding_message_size,
                            );
                        let res = grpc.unary(method, req).await;
                        Ok(res)
                    };
                    Box::pin(fut)
                }
                "/sart.v1.FibManagerApi/GetRoutes" => {
                    #[allow(non_camel_case_types)]
                    struct GetRoutesSvc<T: FibManagerApi>(pub Arc<T>);
                    impl<
                        T: FibManagerApi,
                    > tonic::server::UnaryService<super::GetRoutesRequest>
                    for GetRoutesSvc<T> {
                        type Response = super::GetRoutesResponse;
                        type Future = BoxFuture<
                            tonic::Response<Self::Response>,
                            tonic::Status,
                        >;
                        fn call(
                            &mut self,
                            request: tonic::Request<super::GetRoutesRequest>,
                        ) -> Self::Future {
                            let inner = Arc::clone(&self.0);
                            let fut = async move {
                                <T as FibManagerApi>::get_routes(&inner, request).await
                            };
                            Box::pin(fut)
                        }
                    }
                    let accept_compression_encodings = self.accept_compression_encodings;
                    let send_compression_encodings = self.send_compression_encodings;
                    let max_decoding_message_size = self.max_decoding_message_size;
                    let max_encoding_message_size = self.max_encoding_message_size;
                    let inner = self.inner.clone();
                    let fut = async move {
                        let inner = inner.0;
                        let method = GetRoutesSvc(inner);
                        let codec = tonic::codec::ProstCodec::default();
                        let mut grpc = tonic::server::Grpc::new(codec)
                            .apply_compression_config(
                                accept_compression_encodings,
                                send_compression_encodings,
                            )
                            .apply_max_message_size_config(
                                max_decoding_message_size,
                                max_encoding_message_size,
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
    impl<T: FibManagerApi> Clone for FibManagerApiServer<T> {
        fn clone(&self) -> Self {
            let inner = self.inner.clone();
            Self {
                inner,
                accept_compression_encodings: self.accept_compression_encodings,
                send_compression_encodings: self.send_compression_encodings,
                max_decoding_message_size: self.max_decoding_message_size,
                max_encoding_message_size: self.max_encoding_message_size,
            }
        }
    }
    impl<T: FibManagerApi> Clone for _Inner<T> {
        fn clone(&self) -> Self {
            Self(Arc::clone(&self.0))
        }
    }
    impl<T: std::fmt::Debug> std::fmt::Debug for _Inner<T> {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "{:?}", self.0)
        }
    }
    impl<T: FibManagerApi> tonic::server::NamedService for FibManagerApiServer<T> {
        const NAME: &'static str = "sart.v1.FibManagerApi";
    }
}
