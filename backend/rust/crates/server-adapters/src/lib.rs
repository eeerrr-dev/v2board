use hmac::{Hmac, KeyInit, Mac};
use sha2::Sha256;
use v2board_application::server_runtime::NodeCredentialVerifier;
use v2board_domain_model::ServerKind;

const NODE_TOKEN_PREFIX: &str = "n1_";
const NODE_TOKEN_CONTEXT: &[u8] = b"v2board-server-node-v1\0";

#[derive(Clone, Debug)]
pub struct HmacNodeCredentials {
    master_key: String,
}

impl HmacNodeCredentials {
    pub fn new(master_key: impl Into<String>) -> Self {
        Self {
            master_key: master_key.into(),
        }
    }

    pub fn derive(&self, node_type: &str, node_id: i32, credential_epoch: i64) -> Option<String> {
        derive_node_token(&self.master_key, node_type, node_id, credential_epoch)
    }

    pub fn verify(
        &self,
        node_type: &str,
        node_id: i32,
        credential_epoch: i64,
        candidate: &str,
    ) -> bool {
        verify_node_token(
            &self.master_key,
            node_type,
            node_id,
            credential_epoch,
            candidate,
        )
    }
}

impl NodeCredentialVerifier for HmacNodeCredentials {
    fn verify(&self, kind: ServerKind, node_id: i32, epoch: i64, candidate: &str) -> bool {
        self.verify(kind.as_str(), node_id, epoch, candidate)
    }
}

/// Derive a node-scoped credential from the deployment master key and a
/// revocable per-node epoch. Only the HMAC output is installed on the node;
/// compromising it does not reveal credentials for sibling nodes.
pub fn derive_node_token(
    master_key: &str,
    node_type: &str,
    node_id: i32,
    credential_epoch: i64,
) -> Option<String> {
    if master_key.is_empty() || credential_epoch < 0 {
        return None;
    }
    let mut mac = <Hmac<Sha256> as KeyInit>::new_from_slice(master_key.as_bytes()).ok()?;
    update_mac(&mut mac, node_type, node_id, credential_epoch);
    Some(format!(
        "{NODE_TOKEN_PREFIX}{}",
        hex::encode(mac.finalize().into_bytes())
    ))
}

/// Verify a node-scoped token with the HMAC implementation's constant-time
/// signature comparison.
pub fn verify_node_token(
    master_key: &str,
    node_type: &str,
    node_id: i32,
    credential_epoch: i64,
    candidate: &str,
) -> bool {
    if master_key.is_empty() || credential_epoch < 0 {
        return false;
    }
    let Some(encoded) = candidate.strip_prefix(NODE_TOKEN_PREFIX) else {
        return false;
    };
    let Ok(signature) = hex::decode(encoded) else {
        return false;
    };
    let Ok(mut mac) = <Hmac<Sha256> as KeyInit>::new_from_slice(master_key.as_bytes()) else {
        return false;
    };
    update_mac(&mut mac, node_type, node_id, credential_epoch);
    mac.verify_slice(&signature).is_ok()
}

fn update_mac(mac: &mut Hmac<Sha256>, node_type: &str, node_id: i32, credential_epoch: i64) {
    mac.update(NODE_TOKEN_CONTEXT);
    mac.update(node_type.as_bytes());
    mac.update(&[0]);
    mac.update(node_id.to_string().as_bytes());
    mac.update(&[0]);
    mac.update(credential_epoch.to_string().as_bytes());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn node_tokens_are_bound_to_master_identity_and_epoch() {
        let credentials = HmacNodeCredentials::new("a sufficiently long server master key");
        let token = credentials.derive("v2node", 7, 0).unwrap();
        assert!(credentials.verify("v2node", 7, 0, &token));
        assert!(!credentials.verify("v2node", 8, 0, &token));
        assert!(!credentials.verify("v2node", 7, 1, &token));
        assert!(!credentials.verify("vmess", 7, 0, &token));
        assert!(!credentials.verify("v2node", 7, 0, "a sufficiently long server master key"));
    }
}
