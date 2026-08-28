use std::env;
use std::path::PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let out_dir = env::var("OUT_DIR")?;
    let descriptor_path = PathBuf::from(out_dir).join("reverse_descriptor.bin");

    tonic_build::configure()
        .file_descriptor_set_path(&descriptor_path)
        .compile_protos(&["proto/reverse.proto"], &["proto"])?;
    Ok(())
}
