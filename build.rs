use std::path::Path;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let out_file = Path::new("src/generated/proto.rs");

    // If pre-generated proto file exists and PROTOC isn't set,
    // skip regeneration (enables cross-compilation without protoc)
    if out_file.exists() && std::env::var("PROTOC").is_err() && which_protoc().is_none() {
        println!("cargo:rerun-if-changed=proto/nezha.proto");
        eprintln!("Note: Using pre-generated proto.rs (protoc not found)");
        return Ok(());
    }

    tonic_build::configure()
        .build_server(false)
        .build_client(true)
        .out_dir("src/generated")
        .compile_protos(&["proto/nezha.proto"], &["proto/"])?;
    Ok(())
}

fn which_protoc() -> Option<std::path::PathBuf> {
    std::env::var_os("PATH").and_then(|paths| {
        std::env::split_paths(&paths).find_map(|dir| {
            let full = dir.join("protoc");
            if full.is_file() {
                Some(full)
            } else {
                None
            }
        })
    })
}
