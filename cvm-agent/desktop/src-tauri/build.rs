fn main() {
    // Compile protobuf files
    tonic_build::configure()
        .build_server(false) // Client only
        .compile_protos(&["proto/agent.proto"], &["proto"])
        .expect("Failed to compile protos");

    tauri_build::build()
}
