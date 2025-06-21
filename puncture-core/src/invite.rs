use std::str::FromStr;

use anyhow::{Result, ensure};

pub fn encode(node_id: &iroh::NodeId) -> String {
    format!("pct{}", node_id)
}

pub fn decode(s: &str) -> Result<iroh::NodeId> {
    ensure!(s.starts_with("pct"), "Invalid Prefix");

    Ok(iroh::NodeId::from_str(&s[3..])?)
}
