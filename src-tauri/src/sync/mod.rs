mod app;
mod auth;
mod client;
mod persistence;
mod protocol;
mod receiver;
mod runtime;
mod secrets;
mod server;

#[cfg(test)]
mod tests_integration;
#[cfg(test)]
mod tests_sync;

use std::collections::HashSet;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

use anyhow::anyhow;

use crate::core::{AppError, Result};
use crate::settings::SyncSettings;

pub use app::{init, reconfigure};
pub use runtime::SyncRuntime;

pub(crate) fn validate_configuration(settings: &SyncSettings) -> Result<()> {
    validate_configuration_with_loopback(settings, false)
}

fn validate_configuration_with_loopback(
    settings: &SyncSettings,
    allow_loopback: bool,
) -> Result<()> {
    if !settings.enabled {
        return Ok(());
    }
    if uuid::Uuid::parse_str(&settings.device_id).is_err() {
        return Err(config_error());
    }
    if settings.listen_port == 0 {
        return Err(config_error());
    }
    let bind = parse_sync_address(&settings.bind_address, allow_loopback)?;
    let mut peer_ids = HashSet::with_capacity(settings.peers.len());
    for peer in &settings.peers {
        if uuid::Uuid::parse_str(&peer.id).is_err()
            || peer.id == settings.device_id
            || peer.port == 0
            || peer.secret_reference.trim().is_empty()
            || peer.secret_reference.len() > 256
            || !peer_ids.insert(peer.id.as_str())
        {
            return Err(config_error());
        }
        parse_sync_address(&peer.address, allow_loopback)?;
    }
    if settings
        .peers
        .iter()
        .any(|peer| peer.address == settings.bind_address && peer.port == settings.listen_port)
    {
        return Err(config_error());
    }
    let _ = bind;
    Ok(())
}

fn parse_sync_address(value: &str, allow_loopback: bool) -> Result<IpAddr> {
    let address = value.parse::<IpAddr>().map_err(|_| config_error())?;
    if is_permitted_sync_address(address, allow_loopback) {
        Ok(address)
    } else {
        Err(config_error())
    }
}

fn is_permitted_sync_address(address: IpAddr, allow_loopback: bool) -> bool {
    if address.is_unspecified() || address.is_multicast() {
        return false;
    }
    if address.is_loopback() {
        return allow_loopback;
    }
    match address {
        IpAddr::V4(ip) => is_tailscale_v4(ip),
        IpAddr::V6(ip) => is_tailscale_v6(ip),
    }
}

fn is_tailscale_v4(address: Ipv4Addr) -> bool {
    let value = u32::from(address);
    let start = u32::from(Ipv4Addr::new(100, 64, 0, 0));
    let end = u32::from(Ipv4Addr::new(100, 127, 255, 255));
    (start..=end).contains(&value)
}

fn is_tailscale_v6(address: Ipv6Addr) -> bool {
    let segments = address.segments();
    segments[0] == 0xfd7a && segments[1] == 0x115c && segments[2] == 0xa1e0
}

fn config_error() -> AppError {
    AppError::Other(anyhow!("sync configuration is invalid"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::SyncPeerSettings;

    fn enabled(address: &str) -> SyncSettings {
        SyncSettings {
            enabled: true,
            bind_address: address.to_owned(),
            listen_port: 45_321,
            device_id: uuid::Uuid::new_v4().to_string(),
            peers: Vec::new(),
        }
    }

    #[test]
    fn production_configuration_accepts_only_tailscale_addresses() {
        assert!(validate_configuration(&enabled("100.64.0.1")).is_ok());
        assert!(validate_configuration(&enabled("100.127.255.255")).is_ok());
        assert!(validate_configuration(&enabled("fd7a:115c:a1e0::1")).is_ok());
        for rejected in ["0.0.0.0", "::", "127.0.0.1", "224.0.0.1", "192.168.1.2"] {
            assert!(validate_configuration(&enabled(rejected)).is_err());
        }
    }

    #[test]
    fn localhost_requires_the_explicit_test_path() {
        let settings = enabled("127.0.0.1");
        assert!(validate_configuration(&settings).is_err());
        assert!(validate_configuration_with_loopback(&settings, true).is_ok());
    }

    #[test]
    fn configured_peer_requires_identity_endpoint_and_secret_reference() {
        let mut settings = enabled("100.64.0.1");
        settings.peers.push(SyncPeerSettings {
            id: uuid::Uuid::new_v4().to_string(),
            address: "100.64.0.2".to_owned(),
            port: 45_321,
            secret_reference: String::new(),
        });
        assert!(validate_configuration(&settings).is_err());
    }
}
