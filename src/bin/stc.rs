use clap::Parser;
use simple_tunnel::{client, config, net as stnet, Result};
use std::process::exit;
use tokio::net as tnet;
use tracing::{error, info};

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();
    let args = config::ClientArgs::parse();

    let c = config::load_client_config(&args.config);

    if let Some(config::Commands::GenerateKey {}) = args.command {
        config::generate_key(&args.config, "stc").await?;
        exit(0);
    }

    let mut tries = 5;
    loop {
        let addr = format!("{}:{}", &c.addr, &c.port);
        tries -= 1;
        if tries == 0 {
            error!("connection failed after retries. giving up");
            return Err("".into());
        }

        info!("Handshaking with {}", &addr);
        let client_stream = tnet::TcpStream::connect(&addr).await?;
        let mut client = client::Client::new(&c, client_stream)?;

        if let Err(e) = client.push_tunnel_config().await {
            error!(cause = ?e, "failed to push tunnel config");
            return Err(e);
        };
        // TODO this logic retries if the auth info is bad
        tries = 5;

        if let Err(e) = client.run().await {
            match e {
                stnet::Error::ConnectionDead => {
                    error!(cause = ?e, "client has failed. Reconnecting");
                    continue;
                }
                e => {
                    error!(cause = ?e, "client has failed. Not restarting");
                    return Err(e.into());
                }
            }
        }
    }
}
