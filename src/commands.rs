use crate::responses::NoResponse;
use alloc::string::ToString;
use atat::atat_derive::AtatCmd;
use atat::heapless::String;
use embedded_nal::{SocketAddrV4, SocketAddrV6};

/// Sets the WIFI mode + optionally enables/disables auto_connect
#[derive(Clone, Default, AtatCmd)]
#[at_cmd("+CWMODE", NoResponse, timeout_ms = 1_000)]
pub struct WifiModeCommand {
    /// WIFI mode:
    ///     0: Null mode. Wi-Fi RF will be disabled.
    ///     1: Station mode.
    ///     2: SoftAP mode.
    ///     3: SoftAP+Station mode.
    #[at_arg(position = 0)]
    mode: usize,
}

impl WifiModeCommand {
    pub fn station_mode() -> Self {
        Self { mode: 1 }
    }
}

/// Command for setting the target WIFI access point parameters
#[derive(Clone, Default, AtatCmd)]
#[at_cmd("+CWJAP", NoResponse, timeout_ms = 5_000)]
pub struct AccessPointConnectCommand {
    /// The SSID of the target access point
    #[at_arg(position = 0)]
    ssid: String<32>,

    /// The password/key of the target access point
    #[at_arg(position = 0)]
    password: String<64>,
}

impl AccessPointConnectCommand {
    pub fn new(ssid: String<32>, password: String<64>) -> Self {
        Self { ssid, password }
    }
}

/// Enables/Disables multiple connections
#[derive(Clone, AtatCmd)]
#[at_cmd("+CIPMUX", NoResponse, timeout_ms = 1_000)]
pub struct SetMultipleConnectionsCommand {
    /// 0: single connection, 1: multiple connections
    mode: usize,
}

impl SetMultipleConnectionsCommand {
    /// Enables multiple connections
    pub fn multiple() -> Self {
        Self { mode: 1 }
    }
}

/// Sets the socket receiving mode
#[derive(Clone, AtatCmd)]
#[at_cmd("+CIPRECVMODE", NoResponse, timeout_ms = 1_000)]
pub struct SetSocketReceivingModeCommand {
    /// 0: active mode => ESP-AT will send all the received socket data instantly to the host MCU
    /// 1: passive mode => ESP-AT will keep the received socket data in an internal buffer
    mode: usize,
}

impl SetSocketReceivingModeCommand {
    /// Enables the passive receiving mode
    pub fn passive_mode() -> Self {
        Self { mode: 1 }
    }
}

/// Establish TCP Connection, UDP Transmission, or SSL Connection
#[derive(Clone, AtatCmd)]
#[at_cmd("+CIPSTART", NoResponse, timeout_ms = 5_000, attempts = 1)]
pub struct ConnectCommand {
    /// Socket ID
    link_id: usize,

    /// Connection type, e.g. TCP, TCPv6, SSL, etc.
    connection_type: String<5>,

    /// Remote IPv4 or IPV6 address
    remote_host: String<39>,

    /// Remote port
    port: u16,
}

impl ConnectCommand {
    /// Establishes a IPv4 TCP connection
    pub fn tcp_v4(link_id: usize, remote: SocketAddrV4) -> Self {
        Self {
            link_id,
            connection_type: String::from("TCP"),
            remote_host: String::from(remote.ip().to_string().as_str()),
            port: remote.port(),
        }
    }

    /// Establishes a IPv6 TCP connection
    pub fn tcp_v6(link_id: usize, remote: SocketAddrV6) -> Self {
        Self {
            link_id,
            connection_type: String::from("TCPv6"),
            remote_host: String::from(remote.ip().to_string().as_str()),
            port: remote.port(),
        }
    }
}
