use std::env;
use std::path::PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());

    tonic_build::configure()
        .build_server(true)
        .file_descriptor_set_path(out_dir.join("sartd.bin")) // Add this
        .out_dir("./src")
        .compile(
            &[
                "../../../proto/bgp.proto",
                "../../../proto/cni.proto",
                "../../../proto/fib.proto",
                "../../../proto/fib_manager.proto",
            ],
            &["../../../proto"],
        )
        .unwrap_or_else(|e| panic!("protobuf compile error: {}", e));
    Ok(())
}
