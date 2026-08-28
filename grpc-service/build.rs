fn main() -> Result<(), Box<dyn std::error::Error>> {
    tonic_build::configure()
        .file_descriptor_set_path("proto/reverse_descriptor.bin")
        .compile_protos(&["proto/reverse.proto"], &["proto"])?;
    Ok(())
}
