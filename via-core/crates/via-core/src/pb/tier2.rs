#[derive(Clone, PartialEq, ::prost::Message)]
pub struct SubmitAnomalyBatchRequest {
    #[prost(message, repeated, tag = "1")]
    pub signals: ::prost::alloc::vec::Vec<Tier1Signal>,
}

#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Tier1Signal {
    #[prost(string, tag = "1")]
    pub event_id: ::prost::alloc::string::String,
    #[prost(uint32, tag = "2")]
    pub schema_version: u32,
    #[prost(string, tag = "3")]
    pub entity_hash: ::prost::alloc::string::String,
    #[prost(uint64, tag = "4")]
    pub timestamp: u64,
    #[prost(double, tag = "5")]
    pub score: f64,
    #[prost(double, tag = "6")]
    pub severity: f64,
    #[prost(uint32, tag = "7")]
    pub primary_detector: u32,
    #[prost(uint32, tag = "8")]
    pub detectors_fired: u32,
    #[prost(double, tag = "9")]
    pub confidence: f64,
    #[prost(float, repeated, tag = "10")]
    pub detector_scores: ::prost::alloc::vec::Vec<f32>,
    #[prost(map = "string, string", tag = "11")]
    pub attributes:
        ::std::collections::HashMap<::prost::alloc::string::String, ::prost::alloc::string::String>,
}

#[derive(Clone, PartialEq, ::prost::Message)]
pub struct SubmitAnomalyBatchResponse {
    #[prost(string, tag = "1")]
    pub status: ::prost::alloc::string::String,
    #[prost(string, tag = "2")]
    pub event_id: ::prost::alloc::string::String,
    #[prost(string, tag = "3")]
    pub reason: ::prost::alloc::string::String,
}

pub mod tier2_service_client {
    use tonic::codegen::*;

    #[derive(Debug, Clone)]
    pub struct Tier2ServiceClient<T> {
        inner: tonic::client::Grpc<T>,
    }

    impl Tier2ServiceClient<tonic::transport::Channel> {
        pub async fn connect<D>(dst: D) -> Result<Self, tonic::transport::Error>
        where
            D: TryInto<tonic::transport::Endpoint>,
            D::Error: Into<StdError>,
        {
            let conn = tonic::transport::Endpoint::new(dst)?.connect().await?;
            Ok(Self::new(conn))
        }
    }

    impl<T> Tier2ServiceClient<T>
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

        pub async fn submit_anomaly_batch(
            &mut self,
            request: impl tonic::IntoRequest<super::SubmitAnomalyBatchRequest>,
        ) -> Result<tonic::Response<super::SubmitAnomalyBatchResponse>, tonic::Status> {
            self.inner.ready().await.map_err(|e| {
                tonic::Status::unknown(format!("service was not ready: {}", e.into()))
            })?;
            let path = http::uri::PathAndQuery::from_static(
                "/via.tier2.v1.Tier2Service/SubmitAnomalyBatch",
            );
            let codec = tonic::codec::ProstCodec::default();
            self.inner.unary(request.into_request(), path, codec).await
        }
    }
}
