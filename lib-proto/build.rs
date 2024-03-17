fn main() -> Result<(), Box<dyn std::error::Error>> {
    //tonic_build::compile_protos("market/market.proto")?;
    tonic_build::configure()
        .type_attribute("User", "#[derive(serde::Deserialize, serde::Serialize)]")
        .compile(&["market/market.proto"], &["market"])?;
    Ok(())
}
