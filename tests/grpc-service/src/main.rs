use tonic::{transport::Server, Request, Response, Status};
use tokio_stream::StreamExt;
use tokio_stream::wrappers::ReceiverStream;

pub mod reverse {
    tonic::include_proto!("reverse");
}

use reverse::reverser_server::{Reverser, ReverserServer};
use reverse::{ReverseRequest, ReverseResponse};

#[derive(Debug, Default)]
pub struct MyReverser {}

#[tonic::async_trait]
impl Reverser for MyReverser {
    type ReverseStringsStream = ReceiverStream<Result<ReverseResponse, Status>>;

    async fn reverse_string(
        &self,
        request: Request<ReverseRequest>,
    ) -> Result<Response<ReverseResponse>, Status> {
        let input = request.into_inner().input;
        let output = input.chars().rev().collect();

        let reply = ReverseResponse { output };

        Ok(Response::new(reply))
    }

    async fn reverse_strings(
        &self,
        request: Request<tonic::Streaming<ReverseRequest>>,
    ) -> Result<Response<Self::ReverseStringsStream>, Status> {
        let mut stream = request.into_inner();
        let (tx, rx) = tokio::sync::mpsc::channel(4);

        tokio::spawn(async move {
            while let Some(result) = stream.next().await {
                match result {
                    Ok(req) => {
                        let input = req.input;
                        let output = input.chars().rev().collect();
                        let reply = ReverseResponse { output };
                        if tx.send(Ok(reply)).await.is_err() {
                            break;
                        }
                    }
                    Err(e) => {
                        if tx.send(Err(Status::internal(e.to_string()))).await.is_err() {
                            break;
                        }
                        break;
                    }
                }
            }
        });

        let output_stream = ReceiverStream::new(rx);
        Ok(Response::new(output_stream))
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let addr = "[::1]:50051".parse()?;
    let reverser = MyReverser::default();

    // Use the path relative to the manifest directory
    let descriptor_set = include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/proto/reverse_descriptor.bin"));

    // Create the reflection service
    let reflection_service = tonic_reflection::server::Builder::configure()
        .register_encoded_file_descriptor_set(descriptor_set)
        .build()?;

    println!("ReverserServer listening on {}", addr);

    Server::builder()
        .add_service(ReverserServer::new(reverser))
        .add_service(reflection_service)
        .serve(addr)
        .await?;

    Ok(())
}
