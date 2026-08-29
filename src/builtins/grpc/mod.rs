use crate::CrushResult;
use crate::builtins::grpc::connect::Connect;
use crate::lang::state::scope::Scope;
use std::sync::LazyLock;
use tokio::runtime::Runtime;

mod client;
mod codec;
mod connect;
mod method_call;

static RUNTIME: LazyLock<Runtime> = LazyLock::new(|| {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("Failed to initialize static Tokio runtime")
});

fn runtime() -> &'static Runtime {
    &RUNTIME
}

pub fn declare(root: &Scope) -> CrushResult<()> {
    root.create_namespace(
        "grpc",
        "gRPC connection",
        Box::new(move |grpc| {
            Connect::declare(grpc)?;
            Ok(())
        }),
    )?;
    Ok(())
}
