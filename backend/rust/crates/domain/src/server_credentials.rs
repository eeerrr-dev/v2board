use hmac::{Hmac, KeyInit, Mac};
use sha2::Sha256;

const NODE_TOKEN_PREFIX: &str = "n1_";
const NODE_TOKEN_CONTEXT: &[u8] = b"v2board-server-node-v1\0";

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
    mac.update(NODE_TOKEN_CONTEXT);
    mac.update(node_type.as_bytes());
    mac.update(&[0]);
    mac.update(node_id.to_string().as_bytes());
    mac.update(&[0]);
    mac.update(credential_epoch.to_string().as_bytes());
    Some(format!(
        "{NODE_TOKEN_PREFIX}{}",
        hex::encode(mac.finalize().into_bytes())
    ))
}

/// Constant-time verification for a node-scoped token.
pub fn verify_node_token(
    master_key: &str,
    node_type: &str,
    node_id: i32,
    credential_epoch: i64,
    candidate: &str,
) -> bool {
    let Some(encoded) = candidate.strip_prefix(NODE_TOKEN_PREFIX) else {
        return false;
    };
    let Ok(signature) = hex::decode(encoded) else {
        return false;
    };
    let Ok(mut mac) = <Hmac<Sha256> as KeyInit>::new_from_slice(master_key.as_bytes()) else {
        return false;
    };
    mac.update(NODE_TOKEN_CONTEXT);
    mac.update(node_type.as_bytes());
    mac.update(&[0]);
    mac.update(node_id.to_string().as_bytes());
    mac.update(&[0]);
    mac.update(credential_epoch.to_string().as_bytes());
    mac.verify_slice(&signature).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn node_tokens_are_bound_to_identity_and_epoch() {
        let token =
            derive_node_token("a sufficiently long server master key", "v2node", 7, 0).unwrap();
        assert!(verify_node_token(
            "a sufficiently long server master key",
            "v2node",
            7,
            0,
            &token
        ));
        assert!(!verify_node_token(
            "a sufficiently long server master key",
            "v2node",
            8,
            0,
            &token
        ));
        assert!(!verify_node_token(
            "a sufficiently long server master key",
            "v2node",
            7,
            1,
            &token
        ));
    }
}
