use anyhow::{Context, Result, ensure};
use clap::{Parser, Subcommand};
use serde::Serialize;
use serde_json::Value;

use puncture_cli_core::{
    CloseChannelRequest, ConnectPeerRequest, DisconnectPeerRequest, InviteRequest,
    OnchainDrainRequest, OnchainSendRequest, OpenChannelRequest, ROUTE_LDK_BALANCES,
    ROUTE_LDK_CHANNEL_CLOSE, ROUTE_LDK_CHANNEL_LIST, ROUTE_LDK_CHANNEL_OPEN,
    ROUTE_LDK_CHANNEL_REQUEST, ROUTE_LDK_NODE_ID, ROUTE_LDK_ONCHAIN_DRAIN,
    ROUTE_LDK_ONCHAIN_RECEIVE, ROUTE_LDK_ONCHAIN_SEND, ROUTE_LDK_PEER_CONNECT,
    ROUTE_LDK_PEER_DISCONNECT, ROUTE_LDK_PEER_LIST, ROUTE_USER_INVITE, ROUTE_USER_LIST,
    ROUTE_USER_RECOVER, RecoverRequest, RequestChannelRequest,
};

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Cli {
    /// The port for the Puncture CLI to connect to.
    #[arg(long, env = "CLI_PORT", default_value = "8082")]
    cli_port: u16,

    #[command(subcommand)]
    command: AdminCommands,
}

#[derive(Subcommand, Debug)]
enum AdminCommands {
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
    /// Request a channel from the LSP
    Request(RequestChannelRequest),
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
    /// Generate an invite code
    Invite(InviteRequest),
    /// Recover a user
    Recover(RecoverRequest),
    /// List all users
    List,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        AdminCommands::Ldk { command } => match command {
            AdminLdkCommands::NodeId => request(cli.cli_port, ROUTE_LDK_NODE_ID, ()),
            AdminLdkCommands::Balances => request(cli.cli_port, ROUTE_LDK_BALANCES, ()),
            AdminLdkCommands::Onchain { command } => match command {
                AdminOnchainCommands::Receive => {
                    request(cli.cli_port, ROUTE_LDK_ONCHAIN_RECEIVE, ())
                }
                AdminOnchainCommands::Send(req) => {
                    request(cli.cli_port, ROUTE_LDK_ONCHAIN_SEND, req)
                }
                AdminOnchainCommands::Drain(req) => {
                    request(cli.cli_port, ROUTE_LDK_ONCHAIN_DRAIN, req)
                }
            },
            AdminLdkCommands::Channel { command } => match command {
                AdminChannelCommands::Open(req) => {
                    request(cli.cli_port, ROUTE_LDK_CHANNEL_OPEN, req)
                }
                AdminChannelCommands::Close(req) => {
                    request(cli.cli_port, ROUTE_LDK_CHANNEL_CLOSE, req)
                }
                AdminChannelCommands::List => request(cli.cli_port, ROUTE_LDK_CHANNEL_LIST, ()),
                AdminChannelCommands::Request(req) => {
                    request(cli.cli_port, ROUTE_LDK_CHANNEL_REQUEST, req)
                }
            },
            AdminLdkCommands::Peer { command } => match command {
                AdminPeerCommands::Connect(req) => {
                    request(cli.cli_port, ROUTE_LDK_PEER_CONNECT, req)
                }
                AdminPeerCommands::Disconnect(req) => {
                    request(cli.cli_port, ROUTE_LDK_PEER_DISCONNECT, req)
                }
                AdminPeerCommands::List => request(cli.cli_port, ROUTE_LDK_PEER_LIST, ()),
            },
        },
        AdminCommands::User { command } => match command {
            AdminUserCommands::Invite(req) => request(cli.cli_port, ROUTE_USER_INVITE, req),
            AdminUserCommands::Recover(req) => request(cli.cli_port, ROUTE_USER_RECOVER, req),
            AdminUserCommands::List => request(cli.cli_port, ROUTE_USER_LIST, ()),
        },
    }
}

fn request<R: Serialize>(port: u16, route: &str, request: R) -> Result<()> {
    let response = reqwest::blocking::Client::new()
        .post(format!("http://127.0.0.1:{port}{route}").as_str())
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
