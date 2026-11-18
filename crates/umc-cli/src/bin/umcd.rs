use std::path::PathBuf;
use umc_core::{UmcCore, serve_local_daemon};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let data_dir = std::env::var_os("UMC_DATA_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("umc-data"));
    eprintln!(
        "UMC daemon starting; data directory: {}",
        data_dir.display()
    );
    serve_local_daemon(UmcCore::new(data_dir)).await?;
    Ok(())
}
