pub mod db;
pub mod secret;

use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Result, ensure};
use bitcoin::hex::{DisplayHex, FromHex};
use iroh::NodeId;
use serde::{Deserialize, Serialize};

/// Returns the current time as milliseconds since Unix epoch
pub fn unix_time() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("Time went backwards")
        .as_millis() as i64
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum PunctureCode {
    Invite(InviteCode),
    Recovery(RecoveryCode),
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct InviteCode {
    id: [u8; 16],
    node_id: NodeId,
}

impl InviteCode {
    pub fn id(&self) -> String {
        self.id.as_hex().to_string()
    }

    pub fn node_id(&self) -> NodeId {
        self.node_id
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct RecoveryCode {
    id: [u8; 16],
}

impl RecoveryCode {
    pub fn id(&self) -> String {
        self.id.as_hex().to_string()
    }
}

impl PunctureCode {
    pub fn invite(id: [u8; 16], node_id: NodeId) -> Self {
        Self::Invite(InviteCode { id, node_id })
    }

    pub fn recovery(id: [u8; 16]) -> Self {
        Self::Recovery(RecoveryCode { id })
    }

    pub fn to_invite(&self) -> Result<InviteCode, String> {
        match self {
            PunctureCode::Invite(invite) => Ok(invite.clone()),
            PunctureCode::Recovery(..) => Err("This is a recovery code".to_string()),
        }
    }

    pub fn to_recovery(&self) -> Result<RecoveryCode, String> {
        match self {
            PunctureCode::Invite(..) => Err("This is an invite code".to_string()),
            PunctureCode::Recovery(recovery) => Ok(recovery.clone()),
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
