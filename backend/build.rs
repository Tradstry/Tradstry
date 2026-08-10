fn main() -> Result<(), Box<dyn std::error::Error>> {
    let protoc = protoc_bin_vendored::protoc_bin_path()?;
    let mut config = prost_build::Config::new();
    config.protoc_executable(protoc);
    tonic_prost_build::configure().compile_with_config(
        config,
        &["proto/tradstry/snaptrade/v1/adapter.proto"],
        &["proto"],
    )?;
    println!("cargo:rerun-if-changed=proto/tradstry/snaptrade/v1/adapter.proto");
    Ok(())
}
