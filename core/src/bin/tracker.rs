https://powcoder.com
代写代考加微信 powcoder
Assignment Project Exam Help
Add WeChat powcoder
use bing2bing_core::Tracker;
use clap::Parser;
use dotenvy::dotenv;
use std::net::Ipv4Addr;

#[derive(Debug, Parser, Clone)]
struct Cli {
    /// ip address to bind to (0.0.0.0 should be any ip the tracker can listen on)
    #[clap(long = "host")]
    ip_address: Ipv4Addr,

    /// The port the tracker should listen on.
    #[clap(long)]
    port: u16,
}

#[tokio::main]
async fn main() -> Result<(), bing2bing_core::Bing2BingError> {
    // read in any environment variables set in a .env file
    dotenv().ok();

    // initialize a logger
    tracing_subscriber::fmt::init();

    let args = Cli::parse();

    println!("Tracker starting with args: {:?}", args);
    // let's start up a tracker  and listen.

    let ip_address = &args.ip_address.to_string();
    let port = &args.port.to_string();

    let tracker = Tracker::new(ip_address, &port).await.unwrap();

    tracker.listen().await?;

    Ok(())
}
