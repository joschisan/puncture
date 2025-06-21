use std::process::Command;

use anyhow::{Context, Result, ensure};
use bitcoin::Address;
use bitcoin::secp256k1::PublicKey;
use serde::de::DeserializeOwned;

use puncture_cli_core::{
    BalancesResponse, InviteResponse, NewAddressResponse, OpenChannelResponse,
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

        serde_json::from_str(&output).context(format!("Failed to parse output: {}", output))
    }
}

pub fn onchain_receive(api_port: u16) -> Result<Address> {
    Command::new("target/debug/puncture-cli")
        .arg("--api-url")
        .arg(format!("http://127.0.0.1:{}", api_port))
        .arg("--auth")
        .arg("testing-auth")
        .arg("ldk")
        .arg("onchain")
        .arg("receive")
        .run_puncture_cli::<NewAddressResponse>()
        .map(|response| response.address.assume_checked())
}

pub fn balances(api_port: u16) -> Result<BalancesResponse> {
    Command::new("target/debug/puncture-cli")
        .arg("--api-url")
        .arg(format!("http://127.0.0.1:{}", api_port))
        .arg("--auth")
        .arg("testing-auth")
        .arg("ldk")
        .arg("balances")
        .run_puncture_cli::<BalancesResponse>()
}

pub fn open_channel(api_port_a: u16, node_id_b: PublicKey, ldk_port_b: u16) -> Result<String> {
    Command::new("target/debug/puncture-cli")
        .arg("--api-url")
        .arg(format!("http://127.0.0.1:{}", api_port_a))
        .arg("--auth")
        .arg("testing-auth")
        .arg("ldk")
        .arg("channel")
        .arg("open")
        .arg("--node-id")
        .arg(node_id_b.to_string())
        .arg("--address")
        .arg(format!("127.0.0.1:{}", ldk_port_b))
        .arg("--channel-amount-sats")
        .arg("4000000")
        .arg("--push-to-counterparty-msat")
        .arg("2000000000")
        .run_puncture_cli::<OpenChannelResponse>()
        .map(|response| response.channel_id)
}

pub fn invite(api_port: u16) -> Result<InviteResponse> {
    Command::new("target/debug/puncture-cli")
        .arg("--api-url")
        .arg(format!("http://127.0.0.1:{}", api_port))
        .arg("--auth")
        .arg("testing-auth")
        .arg("invite")
        .run_puncture_cli::<InviteResponse>()
}
