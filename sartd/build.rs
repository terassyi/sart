use std::env;
use std::path::PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());

    tonic_build::configure()
        .build_server(true)
        .file_descriptor_set_path(out_dir.join("bgp.bin")) // Add this
        .out_dir("./src/proto")
        .compile(&["../proto/bgp.proto"], &["../proto"])
        .unwrap_or_else(|e| panic!("protobuf compile error: {}", e));
    Ok(())
}
