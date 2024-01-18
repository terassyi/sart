#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Args {
    #[prost(string, tag = "1")]
    pub container_id: ::prost::alloc::string::String,
    #[prost(string, tag = "2")]
    pub netns: ::prost::alloc::string::String,
    #[prost(string, tag = "3")]
    pub ifname: ::prost::alloc::string::String,
    #[prost(string, repeated, tag = "4")]
    pub path: ::prost::alloc::vec::Vec<::prost::alloc::string::String>,
    #[prost(string, tag = "5")]
    pub args: ::prost::alloc::string::String,
    #[prost(message, optional, tag = "6")]
    pub prev_result: ::core::option::Option<CniResult>,
    #[prost(message, optional, tag = "7")]
    pub conf: ::core::option::Option<SartConf>,
    #[prost(string, tag = "8")]
    pub data: ::prost::alloc::string::String,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct CniResult {
    #[prost(message, repeated, tag = "1")]
    pub interfaces: ::prost::alloc::vec::Vec<Interface>,
    #[prost(message, repeated, tag = "2")]
    pub ips: ::prost::alloc::vec::Vec<IpConf>,
    #[prost(message, repeated, tag = "3")]
    pub routes: ::prost::alloc::vec::Vec<RouteConf>,
    #[prost(message, optional, tag = "4")]
    pub dns: ::core::option::Option<Dns>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct SartConf {}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Interface {
    #[prost(string, tag = "1")]
    pub name: ::prost::alloc::string::String,
    #[prost(string, tag = "2")]
    pub mac: ::prost::alloc::string::String,
    #[prost(string, tag = "3")]
    pub sandbox: ::prost::alloc::string::String,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct IpConf {
    #[prost(uint32, tag = "1")]
    pub interface: u32,
    #[prost(string, tag = "2")]
    pub address: ::prost::alloc::string::String,
    #[prost(string, tag = "3")]
    pub gateway: ::prost::alloc::string::String,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct RouteConf {
    #[prost(string, tag = "1")]
    pub dst: ::prost::alloc::string::String,
    #[prost(string, tag = "2")]
    pub gw: ::prost::alloc::string::String,
    #[prost(int32, tag = "3")]
    pub mtu: i32,
    #[prost(int32, tag = "5")]
    pub advmss: i32,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Dns {
    #[prost(string, repeated, tag = "1")]
    pub nameservers: ::prost::alloc::vec::Vec<::prost::alloc::string::String>,
    #[prost(string, tag = "2")]
    pub domain: ::prost::alloc::string::String,
    #[prost(string, repeated, tag = "3")]
    pub search: ::prost::alloc::vec::Vec<::prost::alloc::string::String>,
    #[prost(string, repeated, tag = "4")]
    pub options: ::prost::alloc::vec::Vec<::prost::alloc::string::String>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ErrorResult {
    #[prost(enumeration = "ErrorCode", tag = "1")]
    pub code: i32,
    #[prost(string, tag = "2")]
    pub msg: ::prost::alloc::string::String,
    #[prost(string, tag = "3")]
    pub details: ::prost::alloc::string::String,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
#[repr(i32)]
pub enum ErrorCode {
    Unknown = 0,
    IncompatibleVersion = 1,
    UnsupportedNetworkConfiguration = 2,
    NotExist = 3,
    InvalidEnvValue = 4,
    IoFailure = 5,
    FailedToDecode = 6,
    InvalidNetworkConfig = 7,
    TryAgainLater = 11,
    AlreadyAdded = 120,
}
impl ErrorCode {
    /// String value of the enum field names used in the ProtoBuf definition.
    ///
    /// The values are not transformed in any way and thus are considered stable
    /// (if the ProtoBuf definition does not change) and safe for programmatic use.
    pub fn as_str_name(&self) -> &'static str {
        match self {
            ErrorCode::Unknown => "Unknown",
            ErrorCode::IncompatibleVersion => "IncompatibleVersion",
            ErrorCode::UnsupportedNetworkConfiguration => {
                "UnsupportedNetworkConfiguration"
            }
            ErrorCode::NotExist => "NotExist",
            ErrorCode::InvalidEnvValue => "InvalidEnvValue",
            ErrorCode::IoFailure => "IOFailure",
            ErrorCode::FailedToDecode => "FailedToDecode",
            ErrorCode::InvalidNetworkConfig => "InvalidNetworkConfig",
            ErrorCode::TryAgainLater => "TryAgainLater",
            ErrorCode::AlreadyAdded => "AlreadyAdded",
        }
    }
    /// Creates an enum from field names used in the ProtoBuf definition.
    pub fn from_str_name(value: &str) -> ::core::option::Option<Self> {
        match value {
            "Unknown" => Some(Self::Unknown),
            "IncompatibleVersion" => Some(Self::IncompatibleVersion),
            "UnsupportedNetworkConfiguration" => {
                Some(Self::UnsupportedNetworkConfiguration)
            }
            "NotExist" => Some(Self::NotExist),
            "InvalidEnvValue" => Some(Self::InvalidEnvValue),
            "IOFailure" => Some(Self::IoFailure),
            "FailedToDecode" => Some(Self::FailedToDecode),
            "InvalidNetworkConfig" => Some(Self::InvalidNetworkConfig),
            "TryAgainLater" => Some(Self::TryAgainLater),
            "AlreadyAdded" => Some(Self::AlreadyAdded),
            _ => None,
        }
    }
}
/// Generated client implementations.
pub mod cni_api_client {
    #![allow(unused_variables, dead_code, missing_docs, clippy::let_unit_value)]
    use tonic::codegen::*;
    use tonic::codegen::http::Uri;
    #[derive(Debug, Clone)]
    pub struct CniApiClient<T> {
        inner: tonic::client::Grpc<T>,
    }
    impl CniApiClient<tonic::transport::Channel> {
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
    impl<T> CniApiClient<T>
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
        ) -> CniApiClient<InterceptedService<T, F>>
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
            CniApiClient::new(InterceptedService::new(inner, interceptor))
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
        pub async fn add(
            &mut self,
            request: impl tonic::IntoRequest<super::Args>,
        ) -> std::result::Result<tonic::Response<super::CniResult>, tonic::Status> {
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
            let path = http::uri::PathAndQuery::from_static("/sart.v1.CNIApi/Add");
            let mut req = request.into_request();
            req.extensions_mut().insert(GrpcMethod::new("sart.v1.CNIApi", "Add"));
            self.inner.unary(req, path, codec).await
        }
        pub async fn del(
            &mut self,
            request: impl tonic::IntoRequest<super::Args>,
        ) -> std::result::Result<tonic::Response<super::CniResult>, tonic::Status> {
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
            let path = http::uri::PathAndQuery::from_static("/sart.v1.CNIApi/Del");
            let mut req = request.into_request();
            req.extensions_mut().insert(GrpcMethod::new("sart.v1.CNIApi", "Del"));
            self.inner.unary(req, path, codec).await
        }
        pub async fn check(
            &mut self,
            request: impl tonic::IntoRequest<super::Args>,
        ) -> std::result::Result<tonic::Response<super::CniResult>, tonic::Status> {
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
            let path = http::uri::PathAndQuery::from_static("/sart.v1.CNIApi/Check");
            let mut req = request.into_request();
            req.extensions_mut().insert(GrpcMethod::new("sart.v1.CNIApi", "Check"));
            self.inner.unary(req, path, codec).await
        }
    }
}
/// Generated server implementations.
pub mod cni_api_server {
    #![allow(unused_variables, dead_code, missing_docs, clippy::let_unit_value)]
    use tonic::codegen::*;
    /// Generated trait containing gRPC methods that should be implemented for use with CniApiServer.
    #[async_trait]
    pub trait CniApi: Send + Sync + 'static {
        async fn add(
            &self,
            request: tonic::Request<super::Args>,
        ) -> std::result::Result<tonic::Response<super::CniResult>, tonic::Status>;
        async fn del(
            &self,
            request: tonic::Request<super::Args>,
        ) -> std::result::Result<tonic::Response<super::CniResult>, tonic::Status>;
        async fn check(
            &self,
            request: tonic::Request<super::Args>,
        ) -> std::result::Result<tonic::Response<super::CniResult>, tonic::Status>;
    }
    #[derive(Debug)]
    pub struct CniApiServer<T: CniApi> {
        inner: _Inner<T>,
        accept_compression_encodings: EnabledCompressionEncodings,
        send_compression_encodings: EnabledCompressionEncodings,
        max_decoding_message_size: Option<usize>,
        max_encoding_message_size: Option<usize>,
    }
    struct _Inner<T>(Arc<T>);
    impl<T: CniApi> CniApiServer<T> {
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
    impl<T, B> tonic::codegen::Service<http::Request<B>> for CniApiServer<T>
    where
        T: CniApi,
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
                "/sart.v1.CNIApi/Add" => {
                    #[allow(non_camel_case_types)]
                    struct AddSvc<T: CniApi>(pub Arc<T>);
                    impl<T: CniApi> tonic::server::UnaryService<super::Args>
                    for AddSvc<T> {
                        type Response = super::CniResult;
                        type Future = BoxFuture<
                            tonic::Response<Self::Response>,
                            tonic::Status,
                        >;
                        fn call(
                            &mut self,
                            request: tonic::Request<super::Args>,
                        ) -> Self::Future {
                            let inner = Arc::clone(&self.0);
                            let fut = async move {
                                <T as CniApi>::add(&inner, request).await
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
                        let method = AddSvc(inner);
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
                "/sart.v1.CNIApi/Del" => {
                    #[allow(non_camel_case_types)]
                    struct DelSvc<T: CniApi>(pub Arc<T>);
                    impl<T: CniApi> tonic::server::UnaryService<super::Args>
                    for DelSvc<T> {
                        type Response = super::CniResult;
                        type Future = BoxFuture<
                            tonic::Response<Self::Response>,
                            tonic::Status,
                        >;
                        fn call(
                            &mut self,
                            request: tonic::Request<super::Args>,
                        ) -> Self::Future {
                            let inner = Arc::clone(&self.0);
                            let fut = async move {
                                <T as CniApi>::del(&inner, request).await
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
                        let method = DelSvc(inner);
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
                "/sart.v1.CNIApi/Check" => {
                    #[allow(non_camel_case_types)]
                    struct CheckSvc<T: CniApi>(pub Arc<T>);
                    impl<T: CniApi> tonic::server::UnaryService<super::Args>
                    for CheckSvc<T> {
                        type Response = super::CniResult;
                        type Future = BoxFuture<
                            tonic::Response<Self::Response>,
                            tonic::Status,
                        >;
                        fn call(
                            &mut self,
                            request: tonic::Request<super::Args>,
                        ) -> Self::Future {
                            let inner = Arc::clone(&self.0);
                            let fut = async move {
                                <T as CniApi>::check(&inner, request).await
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
                        let method = CheckSvc(inner);
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
    impl<T: CniApi> Clone for CniApiServer<T> {
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
    impl<T: CniApi> Clone for _Inner<T> {
        fn clone(&self) -> Self {
            Self(Arc::clone(&self.0))
        }
    }
    impl<T: std::fmt::Debug> std::fmt::Debug for _Inner<T> {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "{:?}", self.0)
        }
    }
    impl<T: CniApi> tonic::server::NamedService for CniApiServer<T> {
        const NAME: &'static str = "sart.v1.CNIApi";
    }
}
