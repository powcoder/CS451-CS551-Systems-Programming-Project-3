https://powcoder.com
代写代考加微信 powcoder
Assignment Project Exam Help
Add WeChat powcoder
use clap::Parser;
use dotenvy::dotenv;

use std::net::Ipv4Addr;

mod simple_tui;

mod fancy_tui;

#[derive(Debug, Parser, Clone)]
pub struct Cli {
    /// What name shoudl this server have?
    #[clap(long)]
    name: String,
    /// server ip address (0.0.0.0 should be any ip the server can listen on)
    #[structopt(long = "host")]
    ip_address: Ipv4Addr,

    /// server port address
    #[structopt(long)]
    port: u16,

    /// tracker ip address
    #[structopt(long = "tracker-host")]
    tracker_ip_address: Ipv4Addr,

    /// tracker port
    #[structopt(long)]
    tracker_port: u16,

    /// maximum number of incomming connections that will be advertised when Announcing to the network.
    #[structopt(default_value = "5")]
    max_connections: u64,

    /// Use simple ui mode? (/say and /quit are the only things that work)
    #[structopt(long)]
    simple: bool,
}

#[tokio::main]
pub async fn main() -> Result<(), bing2bing_core::Bing2BingError> {
    // simple_ui::do_it().await

    // read in any environment variables set in a .env file
    dotenv().ok();

    let args = Cli::parse();

    if args.simple {
        simple_tui::start(args).await
    } else {
        // fancy_tui::start(args).await
        todo!("need to update the fancy tui!")
    }
}

#[derive(Debug)]
pub enum UiClientMessage {
    Say(String),
    Whisper(String, String),
    Ping(String, u64),
}
