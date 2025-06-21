use anyhow::{Context, Result, ensure};
use clap::{Parser, Subcommand};
use serde::Serialize;
use serde_json::Value;

use puncture_cli_core::{
    CLI_BIND_ADDR, CloseChannelRequest, ConnectPeerRequest, DisconnectPeerRequest, InviteRequest,
    OnchainDrainRequest, OnchainSendRequest, OpenChannelRequest, ROUTE_INVITE, ROUTE_LDK_BALANCES,
    ROUTE_LDK_CHANNEL_CLOSE, ROUTE_LDK_CHANNEL_LIST, ROUTE_LDK_CHANNEL_OPEN, ROUTE_LDK_NODE_ID,
    ROUTE_LDK_ONCHAIN_DRAIN, ROUTE_LDK_ONCHAIN_RECEIVE, ROUTE_LDK_ONCHAIN_SEND,
    ROUTE_LDK_PEER_CONNECT, ROUTE_LDK_PEER_DISCONNECT, ROUTE_LDK_PEER_LIST, ROUTE_USER_LIST,
};

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: AdminCommands,
}

#[derive(Subcommand, Debug)]
enum AdminCommands {
    /// Generate an invite code
    Invite(InviteRequest),
    /// LDK node management commands
    Ldk {
        #[command(subcommand)]
        command: AdminLdkCommands,
    },
    /// User management commands
    User {
        #[command(subcommand)]
        command: AdminUserCommands,
    },
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

#[derive(Subcommand, Debug)]
enum AdminUserCommands {
    /// List all users
    List,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        AdminCommands::Invite(req) => request(ROUTE_INVITE, req),
        AdminCommands::Ldk { command } => match command {
            AdminLdkCommands::NodeId => request(ROUTE_LDK_NODE_ID, ()),
            AdminLdkCommands::Balances => request(ROUTE_LDK_BALANCES, ()),
            AdminLdkCommands::Onchain { command } => match command {
                AdminOnchainCommands::Receive => request(ROUTE_LDK_ONCHAIN_RECEIVE, ()),
                AdminOnchainCommands::Send(req) => request(ROUTE_LDK_ONCHAIN_SEND, req),
                AdminOnchainCommands::Drain(req) => request(ROUTE_LDK_ONCHAIN_DRAIN, req),
            },
            AdminLdkCommands::Channel { command } => match command {
                AdminChannelCommands::Open(req) => request(ROUTE_LDK_CHANNEL_OPEN, req),
                AdminChannelCommands::Close(req) => request(ROUTE_LDK_CHANNEL_CLOSE, req),
                AdminChannelCommands::List => request(ROUTE_LDK_CHANNEL_LIST, ()),
            },
            AdminLdkCommands::Peer { command } => match command {
                AdminPeerCommands::Connect(req) => request(ROUTE_LDK_PEER_CONNECT, req),
                AdminPeerCommands::Disconnect(req) => request(ROUTE_LDK_PEER_DISCONNECT, req),
                AdminPeerCommands::List => request(ROUTE_LDK_PEER_LIST, ()),
            },
        },
        AdminCommands::User { command } => match command {
            AdminUserCommands::List => request(ROUTE_USER_LIST, ()),
        },
    }
}

fn request<R: Serialize>(route: &str, request: R) -> Result<()> {
    let response = reqwest::blocking::Client::new()
        .post(format!("http://{}{}", CLI_BIND_ADDR, route).as_str())
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
