use tonic::metadata::MetadataValue;
use tonic::service::Interceptor;
use tonic::{Request, Status};

/// gRPC authentication interceptor that injects client_secret and client_uuid
/// into every RPC call's metadata.
#[derive(Clone, Debug)]
pub struct AuthInterceptor {
    client_secret: String,
    client_uuid: String,
}

impl AuthInterceptor {
    pub fn new(client_secret: String, client_uuid: String) -> Self {
        Self {
            client_secret,
            client_uuid,
        }
    }
}

impl Interceptor for AuthInterceptor {
    fn call(&mut self, mut request: Request<()>) -> Result<Request<()>, Status> {
        let metadata = request.metadata_mut();

        metadata.insert(
            "client_secret",
            MetadataValue::try_from(&self.client_secret)
                .map_err(|e| Status::internal(format!("invalid client_secret metadata: {}", e)))?,
        );

        metadata.insert(
            "client_uuid",
            MetadataValue::try_from(&self.client_uuid)
                .map_err(|e| Status::internal(format!("invalid client_uuid metadata: {}", e)))?,
        );

        Ok(request)
    }
}
