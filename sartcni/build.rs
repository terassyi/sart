use std::env;
use std::path::PathBuf;

fn main() {

    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    let dir = std::fs::read_dir("../proto").unwrap();
    let mut files: Vec<PathBuf> = Vec::new();
    for p in dir.into_iter() {
        files.push(p.unwrap().path());
    }
    println!("{:?}", files);

    tonic_build::configure()
        .build_client(true)
        .file_descriptor_set_path(out_dir.join("sart.bin")) // Add this
        .out_dir("./src/proto")
        .compile(&["../proto/cni.proto"], &["../proto"])
        .unwrap_or_else(|e| panic!("protobuf compile error: {}", e));
}
