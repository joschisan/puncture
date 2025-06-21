use bitcoin::address::NetworkUnchecked;
use bitcoin::secp256k1::PublicKey;
use bitcoin::{Address, OutPoint};
use clap::Args;
use serde::{Deserialize, Serialize};

pub const ROUTE_LDK_NODE_ID: &str = "/ldk/node-id";
pub const ROUTE_LDK_BALANCES: &str = "/ldk/balances";
pub const ROUTE_LDK_ONCHAIN_RECEIVE: &str = "/ldk/onchain/receive";
pub const ROUTE_LDK_ONCHAIN_SEND: &str = "/ldk/onchain/send";
pub const ROUTE_LDK_ONCHAIN_DRAIN: &str = "/ldk/onchain/drain";
pub const ROUTE_LDK_CHANNEL_OPEN: &str = "/ldk/channel/open";
pub const ROUTE_LDK_CHANNEL_CLOSE: &str = "/ldk/channel/close";
pub const ROUTE_LDK_CHANNEL_LIST: &str = "/ldk/channel/list";
pub const ROUTE_LDK_CHANNEL_REQUEST: &str = "/ldk/channel/request";
pub const ROUTE_LDK_PEER_CONNECT: &str = "/ldk/peer/connect";
pub const ROUTE_LDK_PEER_DISCONNECT: &str = "/ldk/peer/disconnect";
pub const ROUTE_LDK_PEER_LIST: &str = "/ldk/peer/list";
pub const ROUTE_USER_INVITE: &str = "/user/invite";
pub const ROUTE_USER_LIST: &str = "/user/list";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeIdResponse {
    /// The node's public key
    pub node_id: PublicKey,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BalancesResponse {
    /// The total balance in the on-chain wallet
    pub total_onchain_balance_sats: u64,
    /// The total inbound capacity across all channels
    pub total_inbound_capacity_msat: u64,
    /// The total outbound capacity across all channels
    pub total_outbound_capacity_msat: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OnchainReceiveResponse {
    /// The generated Bitcoin address
    pub address: Address<NetworkUnchecked>,
}

#[derive(Debug, Clone, Args, Serialize, Deserialize)]
pub struct OnchainSendRequest {
    /// Bitcoin address to send to
    pub address: Address<NetworkUnchecked>,
    /// Amount in satoshis
    pub amount_sats: u64,
    /// The fee rate to use in satoshis per vbyte (optional)
    #[arg(long)]
    pub sats_per_vbyte: Option<u64>,
}

#[derive(Debug, Clone, Args, Serialize, Deserialize)]
pub struct OnchainDrainRequest {
    /// The address to drain the funds to
    pub address: Address<NetworkUnchecked>,
    /// The fee rate to use in satoshis per vbyte (optional)
    #[arg(long)]
    pub sats_per_vbyte: Option<u64>,
}

#[derive(Debug, Clone, Args, Serialize, Deserialize)]
pub struct OpenChannelRequest {
    /// The public key of the node to open a channel with
    pub node_id: PublicKey,
    /// The network address of the node (IP:PORT, HOSTNAME:PORT or Onion address)
    pub socket_address: String,
    /// The amount to fund the channel with, in satoshis
    pub channel_amount_sats: u64,
    /// Amount to push to the counterparty when opening the channel
    #[arg(long)]
    pub push_to_counterparty_msat: Option<u64>,
    /// Whether to announce the channel publicly
    #[arg(long)]
    pub public: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenChannelResponse {
    /// The channel ID in hex encoding
    pub channel_id: String,
}

#[derive(Debug, Clone, Args, Serialize, Deserialize)]
pub struct CloseChannelRequest {
    /// User channel ID in hex encoding
    pub user_channel_id: String,
    /// Counterparty node public key
    pub counterparty_node_id: PublicKey,
    /// Force close the channel
    #[arg(long)]
    pub force: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelInfo {
    /// User channel ID in hex encoding
    pub user_channel_id: String,
    /// Counterparty node public key
    pub counterparty_node_id: PublicKey,
    /// Channel value in satoshis
    pub channel_value_sats: u64,
    /// Whether the channel is outbound
    pub is_outbound: bool,
    /// Outbound capacity in millisatoshis
    pub outbound_capacity_msat: u64,
    /// Inbound capacity in millisatoshis
    pub inbound_capacity_msat: u64,
    /// Whether the channel is ready
    pub is_channel_ready: bool,
    /// Whether the channel is usable
    pub is_usable: bool,
    /// Funding transaction ID
    pub funding_txo: Option<OutPoint>,
    /// Number of confirmations
    pub confirmations: Option<u32>,
    /// Number of confirmations required to be usable
    pub confirmations_required: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListChannelsResponse {
    /// List of channel information
    pub channels: Vec<ChannelInfo>,
}

#[derive(Debug, Clone, Args, Serialize, Deserialize)]
pub struct RequestChannelRequest {
    /// The balance of the LSP in satoshis
    pub lsp_balance_sat: u64,
    /// The balance of the client in satoshis
    #[arg(long, default_value = "0")]
    pub client_balance_sat: u64,
    /// The number of blocks until the channel expires
    #[arg(long, default_value = "13140")]
    pub channel_expiry_blocks: u32,
    /// Whether to announce the channel publicly
    #[arg(long)]
    pub public: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequestChannelResponse {
    /// The BOLT11 invoice
    pub invoice: String,
}

#[derive(Debug, Clone, Args, Serialize, Deserialize)]
pub struct ConnectPeerRequest {
    /// The public key of the peer to connect to
    pub node_id: PublicKey,
    /// The network address of the peer (IP:PORT, HOSTNAME:PORT or Onion address)
    pub socket_address: String,
    /// Whether to persist the connection (reconnect on restart)
    #[arg(long, default_value = "false")]
    pub persist: bool,
}

#[derive(Debug, Clone, Args, Serialize, Deserialize)]
pub struct DisconnectPeerRequest {
    /// The public key of the peer to disconnect from
    pub counterparty_node_id: PublicKey,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerInfo {
    /// The peer's public key
    pub node_id: PublicKey,
    /// The peer's network address (if known)
    pub address: String,
    /// Whether the peer is persisted between restarts
    pub is_persisted: bool,
    /// Whether the peer is currently connected
    pub is_connected: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListPeersResponse {
    /// List of peer information
    pub peers: Vec<PeerInfo>,
}

#[derive(Debug, Clone, Args, Serialize, Deserialize)]
pub struct InviteRequest {
    /// Expiry time in days
    #[arg(long, default_value = "1")]
    pub expiry_days: u32,
    /// Maximum number of users that can register with this invite
    #[arg(long, default_value = "10")]
    pub user_limit: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InviteResponse {
    /// The invite in hex encoding
    pub invite: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserInfo {
    /// The user's public key
    pub user_pk: String,
    /// The user's balance in millisatoshis
    pub balance_msat: u64,
    /// Timestamp in milliseconds since the Unix epoch
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListUsersResponse {
    /// List of user information
    pub users: Vec<UserInfo>,
}
