use anyhow::{Context, Result, ensure};
use clap::{Parser, Subcommand};
use serde::Serialize;
use serde_json::Value;
use url::Url;

use puncture_cli_core::{
    CloseChannelRequest, ConnectPeerRequest, DisconnectPeerRequest, InviteRequest,
    OnchainDrainRequest, OnchainSendRequest, OpenChannelRequest,
};

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: AdminCommands,
}

#[derive(Subcommand, Debug)]
enum AdminCommands {
    /// User management commands
    User {
        #[command(subcommand)]
        command: AdminUserCommands,
    },
    /// LDK node management commands
    Ldk {
        #[command(subcommand)]
        command: AdminLdkCommands,
    },
    /// Generate an invite code
    Invite(InviteRequest),
}

#[derive(Subcommand, Debug)]
enum AdminUserCommands {
    /// List all users
    List,
}

#[derive(Subcommand, Debug)]
enum AdminLdkCommands {
    /// Get the node ID
    NodeId,
    /// Get node balances
    Balances,
    /// On-chain operations
    Onchain {
        #[command(subcommand)]
        command: AdminOnchainCommands,
    },
    /// Channel operations
    Channel {
        #[command(subcommand)]
        command: AdminChannelCommands,
    },
    /// Peer management operations
    Peer {
        #[command(subcommand)]
        command: AdminPeerCommands,
    },
}

#[derive(Subcommand, Debug)]
enum AdminOnchainCommands {
    /// Generate a new Bitcoin address to receive funds
    Receive,
    /// Send Bitcoin to an address
    Send(OnchainSendRequest),
    /// Drain all onchain funds to an address
    Drain(OnchainDrainRequest),
}

#[derive(Subcommand, Debug)]
enum AdminChannelCommands {
    /// Open a Lightning channel
    Open(OpenChannelRequest),
    /// Close a Lightning channel
    Close(CloseChannelRequest),
    /// List all Lightning channels
    List,
}

#[derive(Subcommand, Debug)]
enum AdminPeerCommands {
    /// Connect to a Lightning peer
    Connect(ConnectPeerRequest),
    /// Disconnect from a Lightning peer
    Disconnect(DisconnectPeerRequest),
    /// List all connected peers
    List,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        AdminCommands::User { command } => match command {
            AdminUserCommands::List => request("user/list", ()),
        },
        AdminCommands::Ldk { command } => match command {
            AdminLdkCommands::NodeId => request("ldk/node-id", ()),
            AdminLdkCommands::Balances => request("ldk/balances", ()),
            AdminLdkCommands::Onchain { command } => match command {
                AdminOnchainCommands::Receive => request("ldk/onchain/receive", ()),
                AdminOnchainCommands::Send(req) => request("ldk/onchain/send", req),
                AdminOnchainCommands::Drain(req) => request("ldk/onchain/drain", req),
            },
            AdminLdkCommands::Channel { command } => match command {
                AdminChannelCommands::Open(req) => request("ldk/channel/open", req),
                AdminChannelCommands::Close(req) => request("ldk/channel/close", req),
                AdminChannelCommands::List => request("ldk/channel/list", ()),
            },
            AdminLdkCommands::Peer { command } => match command {
                AdminPeerCommands::Connect(req) => request("ldk/peer/connect", req),
                AdminPeerCommands::Disconnect(req) => request("ldk/peer/disconnect", req),
                AdminPeerCommands::List => request("ldk/peer/list", ()),
            },
        },
        AdminCommands::Invite(req) => request("invite", req),
    }
}

fn request<R: Serialize>(route: &str, request: R) -> Result<()> {
    let request_url = Url::parse("http://127.0.0.1:9090")
        .unwrap()
        .join(route)
        .context("Failed to construct URL")?;

    let response = reqwest::blocking::Client::new()
        .post(request_url)
        .json(&serde_json::to_value(request)?)
        .send()
        .context("Failed to connect to daemon")?;

    ensure!(
        response.status().is_success(),
        "API error ({}): {}",
        response.status().as_u16(),
        response.text()?
    );

    println!(
        "{}",
        serde_json::to_string_pretty(&response.json::<Value>()?)?
    );

    Ok(())
}
