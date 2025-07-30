use std::process::Command;

use anyhow::{Context, Result, ensure};
use bitcoin::secp256k1::PublicKey;
use bitcoin::{Address, Network};
use serde::de::DeserializeOwned;

use puncture_cli_core::{
    BalancesResponse, ChannelInfo, InviteResponse, ListChannelsResponse, ListUsersResponse,
    OnchainReceiveResponse, OpenChannelResponse, RecoverResponse, UserInfo,
};

trait RunPunctureCli {
    fn run_puncture_cli<T: DeserializeOwned>(&mut self) -> Result<T>;
}

impl RunPunctureCli for Command {
    fn run_puncture_cli<T: DeserializeOwned>(&mut self) -> Result<T> {
        let output = self.output().context("Failed to run puncture-cli")?;

        ensure!(
            output.status.success(),
            "Puncture CLI returned non-zero exit code: {} : {}",
            String::from_utf8_lossy(&output.stderr),
            String::from_utf8_lossy(&output.stdout),
        );

        let output = String::from_utf8(output.stdout).context("Failed to convert stdout")?;

        serde_json::from_str(&output).context(format!("Failed to parse output: {output}"))
    }
}

pub fn onchain_receive() -> Result<Address> {
    Command::new("target/debug/puncture-cli")
        .arg("ldk")
        .arg("onchain")
        .arg("receive")
        .run_puncture_cli::<OnchainReceiveResponse>()
        .map(|response| {
            response
                .address
                .require_network(Network::Regtest)
                .expect("Address should be valid for regtest network")
        })
}

pub fn balances() -> Result<BalancesResponse> {
    Command::new("target/debug/puncture-cli")
        .arg("ldk")
        .arg("balances")
        .run_puncture_cli::<BalancesResponse>()
}

pub fn open_channel(node_id_b: PublicKey, ldk_port_b: u16) -> Result<String> {
    Command::new("target/debug/puncture-cli")
        .arg("ldk")
        .arg("channel")
        .arg("open")
        .arg(node_id_b.to_string())
        .arg(format!("127.0.0.1:{ldk_port_b}"))
        .arg("4000000")
        .arg("--push-to-counterparty-msat")
        .arg("2000000000")
        .run_puncture_cli::<OpenChannelResponse>()
        .map(|response| response.channel_id)
}

pub fn list_channels() -> Result<Vec<ChannelInfo>> {
    Command::new("target/debug/puncture-cli")
        .arg("ldk")
        .arg("channel")
        .arg("list")
        .run_puncture_cli::<ListChannelsResponse>()
        .map(|response| response.channels)
}

pub fn invite() -> Result<InviteResponse> {
    Command::new("target/debug/puncture-cli")
        .arg("user")
        .arg("invite")
        .run_puncture_cli::<InviteResponse>()
}

pub fn recover(user_pk: String) -> Result<RecoverResponse> {
    Command::new("target/debug/puncture-cli")
        .arg("user")
        .arg("recover")
        .arg(user_pk)
        .run_puncture_cli::<RecoverResponse>()
}

pub fn list_users() -> Result<Vec<UserInfo>> {
    Command::new("target/debug/puncture-cli")
        .arg("user")
        .arg("list")
        .run_puncture_cli::<ListUsersResponse>()
        .map(|response| response.users)
}
