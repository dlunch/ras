mod mdns;
mod rtsp;

use std::error::Error;

#[async_std::main]
async fn main() -> Result<(), Box<dyn Error>> {
    pretty_env_logger::init();

    rtsp::serve("0.0.0.0".parse()?, 5000).await?;

    Ok(())
}
