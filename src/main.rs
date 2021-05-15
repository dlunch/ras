mod mdns;
mod rtsp;

use std::error::Error;

#[async_std::main]
async fn main() -> Result<(), Box<dyn Error>> {
    rtsp::serve("0.0.0.0".parse()?, 7000).await?;

    Ok(())
}
