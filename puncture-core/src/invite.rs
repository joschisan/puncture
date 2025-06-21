use anyhow::{Result, ensure};
use bitcoin::hex::{DisplayHex, FromHex};
use iroh::NodeId;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum Invite {
    V0(InviteV0),
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct InviteV0 {
    pub id: [u8; 16],
    pub node_id: NodeId,
}

impl Invite {
    pub fn new(id: [u8; 16], node_id: NodeId) -> Self {
        Self::V0(InviteV0 { id, node_id })
    }

    pub fn id(&self) -> String {
        match self {
            Invite::V0(v0) => v0.id.as_hex().to_string(),
        }
    }

    pub fn node_id(&self) -> NodeId {
        match self {
            Invite::V0(v0) => v0.node_id,
        }
    }

    pub fn encode(&self) -> String {
        format!("pct{}", postcard::to_allocvec(self).unwrap().as_hex())
    }

    pub fn decode(s: &str) -> Result<Self> {
        ensure!(s.starts_with("pct"), "Invalid prefix");

        Ok(postcard::from_bytes(&Vec::from_hex(&s[3..])?)?)
    }
}
