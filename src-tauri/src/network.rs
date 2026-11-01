//! Network-interface discovery for multi-link Epic downloads.
//!
//! Hyperion binds one HTTP client to each selected interface's IPv4 address. Windows
//! routes a socket by its source address, so this is enough to make each stream leave
//! through the intended adapter. Auto mode keeps one adapter per subnet: Wi-Fi and
//! Ethernet plugged into the same router share an upstream and cannot add bandwidth.

use crate::models::AppSettings;
use network_interface::{Addr, NetworkInterface, NetworkInterfaceConfig};
use serde::{Deserialize, Serialize};
use std::net::Ipv4Addr;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NetworkInfo {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub ipv4: String,
    pub netmask: Option<String>,
    pub subnet: Option<String>,
    pub is_virtual: bool,
}

/// Adapter names that are normally virtual/VPN/tunnel devices. They are shown in the
/// selector but Auto mode leaves them alone; users can still pick one explicitly.
fn is_virtual_name(name: &str) -> bool {
    let name = name.to_ascii_lowercase();
    [
        "loopback",
        "vethernet",
        "hyper-v",
        "wsl",
        "docker",
        "vmware",
        "virtualbox",
        "tailscale",
        "zerotier",
        "wireguard",
        "wintun",
        "openvpn",
        "vpn",
        "tap-",
        "tun",
        "radmin",
        "hamachi",
    ]
    .iter()
    .any(|needle| name.contains(needle))
}

fn kind_for(name: &str) -> String {
    let name = name.to_ascii_lowercase();
    if ["wi-fi", "wifi", "wlan", "wireless"].iter().any(|n| name.contains(n)) {
        "wifi".to_string()
    } else if ["rndis", "ndis", "tether", "iphone", "ipad", "android", "usb"]
        .iter()
        .any(|n| name.contains(n))
    {
        "usb".to_string()
    } else if name.contains("ethernet")
        || name.starts_with("eth")
        || name == "lan"
        || name.contains("local area")
    {
        "ethernet".to_string()
    } else {
        "other".to_string()
    }
}

fn subnet(ip: Ipv4Addr, mask: Ipv4Addr) -> String {
    let network = u32::from(ip) & u32::from(mask);
    format!("{}/{}", Ipv4Addr::from(network), u32::from(mask).count_ones())
}

/// Every usable IPv4 interface address, including virtual adapters.
pub fn list_networks() -> Vec<NetworkInfo> {
    let interfaces = match NetworkInterface::show() {
        Ok(interfaces) => interfaces,
        Err(error) => {
            tracing::warn!("Could not enumerate network interfaces: {}", error);
            return Vec::new();
        }
    };

    let mut networks = Vec::new();
    for interface in interfaces {
        if interface.internal {
            continue;
        }
        for address in interface.addr {
            let Addr::V4(v4) = address else {
                continue;
            };
            if v4.ip.is_loopback() || v4.ip.is_link_local() || v4.ip.is_unspecified() {
                continue;
            }
            let Some(mask) = v4.netmask else {
                continue;
            };
            let id = format!("{}|{}", interface.name, v4.ip);
            networks.push(NetworkInfo {
                id,
                name: interface.name.clone(),
                kind: kind_for(&interface.name),
                ipv4: v4.ip.to_string(),
                netmask: Some(mask.to_string()),
                subnet: Some(subnet(v4.ip, mask)),
                is_virtual: is_virtual_name(&interface.name),
            });
        }
    }

    networks.sort_by(|a, b| {
        (a.subnet.clone().unwrap_or_default(), &a.id)
            .cmp(&(b.subnet.clone().unwrap_or_default(), &b.id))
    });
    networks.dedup_by(|a, b| a.id == b.id);
    networks
}

/// Interfaces that should carry download traffic for the current settings.
///
/// `auto` chooses one physical adapter per subnet; `selected` uses exactly the listed
/// IDs; `off` returns none. If auto finds nothing, the caller falls back to the legacy
/// default-route client.
pub fn selected_networks(settings: &AppSettings) -> Vec<NetworkInfo> {
    select_from(list_networks(), &settings.network_mode, &settings.network_ids)
}

fn select_from(
    all: Vec<NetworkInfo>,
    mode: &str,
    ids: &[String],
) -> Vec<NetworkInfo> {
    match mode {
        "off" => Vec::new(),
        "selected" => all
            .into_iter()
            .filter(|network| ids.iter().any(|id| id == &network.id))
            .collect(),
        _ => {
            let mut selected = Vec::new();
            let mut seen_subnets = std::collections::HashSet::new();
            for network in all.into_iter().filter(|network| !network.is_virtual) {
                let key = network
                    .subnet
                    .clone()
                    .unwrap_or_else(|| network.id.clone());
                if seen_subnets.insert(key) {
                    selected.push(network);
                }
            }
            selected
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn virtual_names_are_recognized() {
        assert!(is_virtual_name("vEthernet (Default Switch)"));
        assert!(is_virtual_name("Tailscale"));
        assert!(is_virtual_name("WireGuard Tunnel"));
        assert!(!is_virtual_name("Wi-Fi"));
        assert!(!is_virtual_name("Ethernet 2"));
    }

    #[test]
    fn subnet_masks_ipv4() {
        assert_eq!(
            subnet(Ipv4Addr::new(192, 168, 1, 42), Ipv4Addr::new(255, 255, 255, 0)),
            "192.168.1.0/24"
        );
        assert_eq!(
            subnet(Ipv4Addr::new(10, 20, 30, 40), Ipv4Addr::new(255, 255, 0, 0)),
            "10.20.0.0/16"
        );
    }

    #[test]
    fn kind_labels_cover_the_common_names() {
        assert_eq!(kind_for("Wi-Fi"), "wifi");
        assert_eq!(kind_for("Ethernet 2"), "ethernet");
        assert_eq!(kind_for("Remote NDIS based Internet Sharing Device"), "usb");
        assert_eq!(kind_for("Something Else"), "other");
    }

    fn network(name: &str, ip: &str, subnet: &str, is_virtual: bool) -> NetworkInfo {
        NetworkInfo {
            id: format!("{name}|{ip}"),
            name: name.to_string(),
            kind: "other".to_string(),
            ipv4: ip.to_string(),
            netmask: Some("255.255.255.0".to_string()),
            subnet: Some(subnet.to_string()),
            is_virtual,
        }
    }

    #[test]
    fn auto_selects_one_adapter_per_subnet() {
        let all = vec![
            network("Ethernet", "192.168.1.10", "192.168.1.0/24", false),
            network("Wi-Fi", "192.168.1.11", "192.168.1.0/24", false),
            network("Ethernet 2", "10.0.0.10", "10.0.0.0/24", false),
        ];
        let selected = select_from(all, "auto", &[]);
        assert_eq!(selected.len(), 2);
        assert!(selected.iter().any(|network| network.ipv4 == "192.168.1.10"));
        assert!(selected.iter().any(|network| network.ipv4 == "10.0.0.10"));
    }

    #[test]
    fn auto_skips_virtual_adapters_but_selected_can_use_them() {
        let all = vec![
            network("Ethernet", "192.168.1.10", "192.168.1.0/24", false),
            network("Tailscale", "100.64.0.2", "100.64.0.0/24", true),
        ];
        assert_eq!(select_from(all.clone(), "auto", &[]).len(), 1);
        let ids = vec!["Tailscale|100.64.0.2".to_string()];
        let selected = select_from(all, "selected", &ids);
        assert_eq!(selected.len(), 1);
        assert_eq!(selected[0].name, "Tailscale");
    }

    #[test]
    fn off_selects_nothing() {
        let all = vec![network("Ethernet", "192.168.1.10", "192.168.1.0/24", false)];
        assert!(select_from(all, "off", &[]).is_empty());
    }
}
