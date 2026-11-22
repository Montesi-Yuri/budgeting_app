//! Finding a paired Device on the local network (sync-design.md §3.4): while
//! Sync is on, the listening Device (the PC) announces itself with mDNS as
//! `_bsync._tcp.local.`, and a Device whose last known address fails looks
//! for it by its device id.
//!
//! An announcement only says where to try: the Noise handshake still checks
//! the Device's key, so a fake announcement gets nothing.

use std::net::Ipv4Addr;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use mdns_sd::{IfKind, ServiceDaemon, ServiceEvent, ServiceInfo};

use crate::error::{Error, Result};

/// The mDNS service type of a listening Device.
pub const SERVICE_TYPE: &str = "_bsync._tcp.local.";

/// The standard mDNS port.
pub const MDNS_PORT: u16 = 5353;

/// The TXT property holding the announcing Device's device id.
const ID_KEY: &str = "id";

/// How a Device announces itself and finds the others.
pub trait Discovery: Send + Sync {
    /// Announce this Device, listening on `port`, until the returned
    /// [`Announcement`] is dropped.
    fn announce(&self, device_id: &str, name: &str, port: u16) -> Result<Announcement>;

    /// The addresses ("ip:port") where the Device `device_id` announces
    /// itself, waiting at most `timeout`. Empty if it wasn't found.
    fn find(&self, device_id: &str, timeout: Duration) -> Vec<String>;
}

/// An announcement in progress: it stops when dropped.
pub struct Announcement {
    _guard: Box<dyn Send>,
}

impl Announcement {
    /// An announcement that ends when `guard` is dropped.
    pub fn new(guard: impl Send + 'static) -> Announcement {
        Announcement {
            _guard: Box::new(guard),
        }
    }
}

/// [`Discovery`] with multicast DNS (crate `mdns-sd`, which runs its own
/// thread, no async runtime). The mDNS socket is opened on first use, so a
/// Device that never announces or looks for another opens nothing.
pub struct Mdns {
    port: u16,
    loopback_only: bool,
    daemon: Mutex<Option<ServiceDaemon>>,
}

impl Mdns {
    /// mDNS on the standard port, on the IPv4 network interfaces (not
    /// loopback).
    pub fn new() -> Mdns {
        Mdns {
            port: MDNS_PORT,
            loopback_only: false,
            daemon: Mutex::new(None),
        }
    }

    /// mDNS on the loopback interface only, on another UDP port: for tests
    /// in one process, without sending anything on the real network. Both
    /// sides must use the same port.
    pub fn loopback_only(port: u16) -> Mdns {
        Mdns {
            port,
            loopback_only: true,
            daemon: Mutex::new(None),
        }
    }

    fn daemon(&self) -> Result<ServiceDaemon> {
        let mut daemon = self.daemon.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(d) = daemon.as_ref() {
            return Ok(d.clone());
        }
        let d = ServiceDaemon::new_with_port(self.port).map_err(net)?;
        // The Sync addresses are "ip:port" with IPv4 (as in the QR code).
        d.disable_interface(IfKind::IPv6).map_err(net)?;
        d.disable_interface(IfKind::LoopbackV6).map_err(net)?;
        if self.loopback_only {
            d.disable_interface(IfKind::IPv4).map_err(net)?;
            d.enable_interface(IfKind::LoopbackV4).map_err(net)?;
        } else {
            d.disable_interface(IfKind::LoopbackV4).map_err(net)?;
        }
        *daemon = Some(d.clone());
        Ok(d)
    }
}

impl Default for Mdns {
    fn default() -> Self {
        Mdns::new()
    }
}

impl Drop for Mdns {
    fn drop(&mut self) {
        if let Some(d) = self.daemon.get_mut().unwrap_or_else(|e| e.into_inner()) {
            let _ = d.shutdown();
        }
    }
}

struct Registered {
    daemon: ServiceDaemon,
    fullname: String,
}

impl Drop for Registered {
    fn drop(&mut self) {
        if let Ok(done) = self.daemon.unregister(&self.fullname) {
            // Wait briefly for the goodbye packet, so turning Sync off is
            // seen at once by the other Devices.
            let _ = done.recv_timeout(Duration::from_secs(1));
        }
    }
}

impl Discovery for Mdns {
    fn announce(&self, device_id: &str, name: &str, port: u16) -> Result<Announcement> {
        let daemon = self.daemon()?;
        let host = format!("{device_id}.local.");
        let properties = [(ID_KEY, device_id), ("name", name)];
        // No fixed addresses: they follow the PC's addresses, also when
        // they change while Sync is on (DHCP, another network).
        let info = ServiceInfo::new(SERVICE_TYPE, device_id, &host, "", port, &properties[..])
            .map_err(net)?
            .enable_addr_auto();
        let fullname = info.get_fullname().to_owned();
        daemon.register(info).map_err(net)?;
        Ok(Announcement::new(Registered { daemon, fullname }))
    }

    fn find(&self, device_id: &str, timeout: Duration) -> Vec<String> {
        let Ok(daemon) = self.daemon() else {
            return Vec::new();
        };
        let Ok(events) = daemon.browse(SERVICE_TYPE) else {
            return Vec::new();
        };
        let deadline = Instant::now() + timeout;
        let mut found = Vec::new();
        while let Some(left) = deadline.checked_duration_since(Instant::now()) {
            let Ok(event) = events.recv_timeout(left) else {
                break;
            };
            let ServiceEvent::ServiceResolved(service) = event else {
                continue;
            };
            if service.get_properties().get_property_val_str(ID_KEY) != Some(device_id) {
                continue;
            }
            let mut ips: Vec<Ipv4Addr> = service.get_addresses_v4().into_iter().collect();
            ips.sort_by_key(|ip| (preference(*ip), *ip));
            found = ips
                .into_iter()
                .map(|ip| format!("{ip}:{}", service.get_port()))
                .collect();
            if !found.is_empty() {
                break;
            }
        }
        let _ = daemon.stop_browse(SERVICE_TYPE);
        found
    }
}

/// Private LAN addresses first, loopback last.
fn preference(ip: Ipv4Addr) -> u8 {
    if ip.is_private() {
        0
    } else if ip.is_loopback() {
        2
    } else {
        1
    }
}

fn net(e: mdns_sd::Error) -> Error {
    Error::Net(format!("mDNS: {e}"))
}
