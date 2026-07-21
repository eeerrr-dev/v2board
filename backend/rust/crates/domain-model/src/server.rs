//! Pure invariants shared by the administrative server-control use cases.

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum ServerKind {
    Shadowsocks,
    Vmess,
    Trojan,
    Tuic,
    Hysteria,
    Vless,
    Anytls,
    V2node,
}

impl ServerKind {
    pub const ALL: [Self; 8] = [
        Self::Shadowsocks,
        Self::Vmess,
        Self::Trojan,
        Self::Tuic,
        Self::Hysteria,
        Self::Vless,
        Self::Anytls,
        Self::V2node,
    ];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Shadowsocks => "shadowsocks",
            Self::Vmess => "vmess",
            Self::Trojan => "trojan",
            Self::Tuic => "tuic",
            Self::Hysteria => "hysteria",
            Self::Vless => "vless",
            Self::Anytls => "anytls",
            Self::V2node => "v2node",
        }
    }
}

impl TryFrom<&str> for ServerKind {
    type Error = ServerInputViolation;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "shadowsocks" => Ok(Self::Shadowsocks),
            "vmess" => Ok(Self::Vmess),
            "trojan" => Ok(Self::Trojan),
            "tuic" => Ok(Self::Tuic),
            "hysteria" => Ok(Self::Hysteria),
            "vless" => Ok(Self::Vless),
            "anytls" => Ok(Self::Anytls),
            "v2node" => Ok(Self::V2node),
            _ => Err(ServerInputViolation::InvalidServerType),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ServerRouteAction {
    Block,
    BlockIp,
    BlockPort,
    Protocol,
    Dns,
    Route,
    RouteIp,
    DefaultOut,
}

impl ServerRouteAction {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Block => "block",
            Self::BlockIp => "block_ip",
            Self::BlockPort => "block_port",
            Self::Protocol => "protocol",
            Self::Dns => "dns",
            Self::Route => "route",
            Self::RouteIp => "route_ip",
            Self::DefaultOut => "default_out",
        }
    }
}

impl TryFrom<&str> for ServerRouteAction {
    type Error = ServerInputViolation;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "block" => Ok(Self::Block),
            "block_ip" => Ok(Self::BlockIp),
            "block_port" => Ok(Self::BlockPort),
            "protocol" => Ok(Self::Protocol),
            "dns" => Ok(Self::Dns),
            "route" => Ok(Self::Route),
            "route_ip" => Ok(Self::RouteIp),
            "default_out" => Ok(Self::DefaultOut),
            _ => Err(ServerInputViolation::InvalidRouteAction),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ServerInputViolation {
    InvalidServerType,
    Required(&'static str),
    UnsupportedField(&'static str),
    InvalidPort(&'static str),
    InvalidRate,
    InvalidGroupIds,
    EmptyGroupIds,
    EmptyRouteRemarks,
    InvalidRouteAction,
    EmptyRouteMatches,
    InvalidTlsSettings,
}

pub fn canonical_server_group_ids(ids: &[i64]) -> Result<Vec<i32>, ServerInputViolation> {
    if ids.is_empty() {
        return Err(ServerInputViolation::EmptyGroupIds);
    }
    let mut canonical = ids
        .iter()
        .copied()
        .map(|id| i32::try_from(id).ok().filter(|id| *id > 0))
        .collect::<Option<Vec<_>>>()
        .ok_or(ServerInputViolation::InvalidGroupIds)?;
    canonical.sort_unstable();
    canonical.dedup();
    Ok(canonical)
}

pub fn validate_server_port(value: i64, field: &'static str) -> Result<i32, ServerInputViolation> {
    if !(1..=65_535).contains(&value) {
        return Err(ServerInputViolation::InvalidPort(field));
    }
    Ok(value as i32)
}

pub fn filter_server_route_matches(rules: &[String]) -> Vec<String> {
    rules
        .iter()
        .filter(|rule| !rule.is_empty() && rule.as_str() != "0")
        .cloned()
        .collect()
}

pub fn validate_server_route_matches(
    action: ServerRouteAction,
    rules: &[String],
) -> Result<Vec<String>, ServerInputViolation> {
    if action == ServerRouteAction::DefaultOut {
        return Ok(Vec::new());
    }
    if rules.is_empty() {
        return Err(ServerInputViolation::EmptyRouteMatches);
    }
    Ok(filter_server_route_matches(rules))
}

pub fn server_available_status(
    now: i64,
    last_check_at: Option<i64>,
    last_push_at: Option<i64>,
) -> i16 {
    let stale_before = now.saturating_sub(300);
    if stale_before >= last_check_at.unwrap_or_default() {
        0
    } else if stale_before >= last_push_at.unwrap_or_default() {
        1
    } else {
        2
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn group_ids_are_positive_bounded_and_canonical_for_locking() {
        assert_eq!(canonical_server_group_ids(&[3, 1, 3]).unwrap(), [1, 3]);
        assert_eq!(
            canonical_server_group_ids(&[]),
            Err(ServerInputViolation::EmptyGroupIds)
        );
        assert_eq!(
            canonical_server_group_ids(&[0]),
            Err(ServerInputViolation::InvalidGroupIds)
        );
        assert_eq!(
            canonical_server_group_ids(&[i64::from(i32::MAX) + 1]),
            Err(ServerInputViolation::InvalidGroupIds)
        );
    }

    #[test]
    fn route_rules_keep_the_recorded_php_falsy_filter() {
        assert_eq!(
            validate_server_route_matches(
                ServerRouteAction::Block,
                &["".into(), "0".into(), "1.1.1.1".into()]
            )
            .unwrap(),
            ["1.1.1.1"]
        );
        assert_eq!(
            validate_server_route_matches(ServerRouteAction::DefaultOut, &["ignored".into()])
                .unwrap(),
            Vec::<String>::new()
        );
    }

    #[test]
    fn availability_matches_control_plane_check_then_push_precedence() {
        assert_eq!(server_available_status(1_000, None, None), 0);
        assert_eq!(server_available_status(1_000, Some(699), None), 0);
        assert_eq!(server_available_status(1_000, Some(701), None), 1);
        assert_eq!(server_available_status(1_000, Some(701), Some(701)), 2);
    }
}
