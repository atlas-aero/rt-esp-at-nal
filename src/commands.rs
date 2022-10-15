use core::fmt::Write;

use crate::responses::LocalAddressResponse;
use crate::responses::NoResponse;
use crate::stack::Error as StackError;
use crate::wifi::{AddressErrors, JoinError};
use atat::atat_derive::AtatCmd;
use atat::heapless::{String, Vec};
use atat::{AtatCmd, Error as AtError, InternalError};
use embedded_nal::{Ipv4Addr, Ipv6Addr, SocketAddrV4, SocketAddrV6};
use numtoa::NumToA;

const MAX_IP_LENGTH: usize = 39; // IPv4: 15, IPv6: 39

/// Trait for mapping command errors
pub trait CommandErrorHandler {
    type Error;

    /// Maps an unexpected WouldBlock error
    const WOULD_BLOCK_ERROR: Self::Error;

    /// Maps regular errors
    fn command_error(&self, error: AtError) -> Self::Error;
}

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

impl CommandErrorHandler for WifiModeCommand {
    type Error = JoinError;
    const WOULD_BLOCK_ERROR: Self::Error = JoinError::UnexpectedWouldBlock;

    fn command_error(&self, error: AtError) -> Self::Error {
        JoinError::ModeError(error)
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

impl CommandErrorHandler for AccessPointConnectCommand {
    type Error = JoinError;

    const WOULD_BLOCK_ERROR: Self::Error = JoinError::UnexpectedWouldBlock;

    fn command_error(&self, error: AtError) -> Self::Error {
        JoinError::ConnectError(error)
    }
}

/// Command for receiving local address information including IP and MAC
#[derive(Clone, AtatCmd)]
#[at_cmd("+CIFSR", Vec<LocalAddressResponse, 4>, timeout_ms = 5_000)]
pub struct ObtainLocalAddressCommand {}

impl ObtainLocalAddressCommand {
    pub fn new() -> Self {
        Self {}
    }
}

impl CommandErrorHandler for ObtainLocalAddressCommand {
    type Error = AddressErrors;
    const WOULD_BLOCK_ERROR: Self::Error = AddressErrors::UnexpectedWouldBlock;

    fn command_error(&self, error: AtError) -> Self::Error {
        AddressErrors::CommandError(error)
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

impl CommandErrorHandler for SetMultipleConnectionsCommand {
    type Error = StackError;
    const WOULD_BLOCK_ERROR: Self::Error = StackError::UnexpectedWouldBlock;

    fn command_error(&self, error: AtError) -> Self::Error {
        StackError::EnablingMultiConnectionsFailed(error)
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

impl CommandErrorHandler for SetSocketReceivingModeCommand {
    type Error = StackError;
    const WOULD_BLOCK_ERROR: Self::Error = StackError::UnexpectedWouldBlock;

    fn command_error(&self, error: AtError) -> Self::Error {
        StackError::EnablingPassiveSocketModeFailed(error)
    }
}

/// Establish TCP Connection, UDP Transmission, or SSL Connection
#[derive(Clone, AtatCmd)]
#[at_cmd("+CIPSTART", NoResponse, timeout_ms = 5_000)]
pub struct ConnectCommand {
    /// Socket ID
    link_id: usize,

    /// Connection type, e.g. TCP, TCPv6, SSL, etc.
    connection_type: String<5>,

    /// Remote IPv4 or IPV6 address
    remote_host: String<MAX_IP_LENGTH>,

    /// Remote port
    port: u16,
}

/// Convert a `IPv4Addr` to a heapless `String`
fn ipv4_to_string(ip: &Ipv4Addr) -> String<MAX_IP_LENGTH> {
    let mut ip_string = String::new();
    let mut num_buf = [0u8; 3];
    for (i, octet) in ip.octets().iter().enumerate() {
        ip_string.write_str(octet.numtoa_str(10, &mut num_buf)).unwrap();
        if i != 3 {
            ip_string.write_char('.').unwrap();
        }
    }
    ip_string
}

/// Convert a `SocketAddrV6` IP to a heapless `String`
fn ipv6_to_string(ip: &Ipv6Addr) -> String<MAX_IP_LENGTH> {
    let mut ip_string = String::new();
    let mut hex_buf = [0u8; 4];
    for (i, segment) in ip.segments().iter().enumerate() {
        // Write segment (hexadectet)
        if segment == &0 {
            // All-zero-segments can be shortened
            ip_string.write_str("0").unwrap()
        } else {
            // Hex-encode IPv6 segment
            base16::encode_config_slice(&segment.to_be_bytes(), base16::EncodeLower, &mut hex_buf);
            ip_string
                // Safety: The result from hex-encoding will always be valid UTF-8
                .write_str(unsafe { core::str::from_utf8_unchecked(&hex_buf) })
                .unwrap();
        }

        // Write separator
        if i != 7 {
            ip_string.write_char(':').unwrap();
        }
    }
    ip_string
}

impl ConnectCommand {
    /// Establishes a IPv4 TCP connection
    pub fn tcp_v4(link_id: usize, remote: SocketAddrV4) -> Self {
        Self {
            link_id,
            connection_type: String::from("TCP"),
            remote_host: ipv4_to_string(remote.ip()),
            port: remote.port(),
        }
    }

    /// Establishes a IPv6 TCP connection
    pub fn tcp_v6(link_id: usize, remote: SocketAddrV6) -> Self {
        Self {
            link_id,
            connection_type: String::from("TCPv6"),
            remote_host: ipv6_to_string(remote.ip()),
            port: remote.port(),
        }
    }
}

impl CommandErrorHandler for ConnectCommand {
    type Error = StackError;
    const WOULD_BLOCK_ERROR: Self::Error = StackError::UnexpectedWouldBlock;

    fn command_error(&self, error: AtError) -> Self::Error {
        StackError::ConnectError(error)
    }
}

/// Initiates the transmission of data
#[derive(Clone, AtatCmd)]
#[at_cmd("+CIPSEND", NoResponse, timeout_ms = 1_000)]
pub struct TransmissionPrepareCommand {
    /// Socket ID
    link_id: usize,

    /// Length of the data to transmit
    length: usize,
}

impl TransmissionPrepareCommand {
    pub fn new(link_id: usize, length: usize) -> Self {
        Self { link_id, length }
    }
}

impl CommandErrorHandler for TransmissionPrepareCommand {
    type Error = StackError;
    const WOULD_BLOCK_ERROR: Self::Error = StackError::UnexpectedWouldBlock;

    fn command_error(&self, error: AtError) -> Self::Error {
        StackError::TransmissionStartFailed(error)
    }
}

/// The actual transmission of data. Max. data length: 256 bytes
pub struct TransmissionCommand<'a> {
    data: &'a [u8],
}

impl<'a> TransmissionCommand<'a> {
    pub fn new(data: &'a [u8]) -> Self {
        Self { data }
    }
}

impl<'a, const LEN: usize> AtatCmd<LEN> for TransmissionCommand<'a> {
    type Response = NoResponse;

    const MAX_TIMEOUT_MS: u32 = 5000;
    const EXPECTS_RESPONSE_CODE: bool = false;

    fn as_bytes(&self) -> Vec<u8, LEN> {
        Vec::from_slice(self.data).unwrap()
    }

    fn parse(&self, _resp: Result<&[u8], InternalError>) -> Result<Self::Response, AtError> {
        Ok(NoResponse {})
    }
}

impl<'a> CommandErrorHandler for TransmissionCommand<'a> {
    type Error = StackError;
    const WOULD_BLOCK_ERROR: Self::Error = StackError::UnexpectedWouldBlock;

    fn command_error(&self, error: AtError) -> Self::Error {
        StackError::SendFailed(error)
    }
}

/// Command for receiving data
#[derive(Clone, AtatCmd)]
#[at_cmd("+CIPRECVDATA", NoResponse, timeout_ms = 1_000)]
pub struct ReceiveDataCommand<const RESP_LEN: usize> {
    /// Socket ID
    link_id: usize,

    /// Length in bytes to receive
    length: usize,
}

impl<const RESP_LEN: usize> ReceiveDataCommand<RESP_LEN> {
    pub fn new(link_id: usize, length: usize) -> Self {
        Self { link_id, length }
    }
}

impl<const RESP_LEN: usize> CommandErrorHandler for ReceiveDataCommand<RESP_LEN> {
    type Error = StackError;
    const WOULD_BLOCK_ERROR: Self::Error = StackError::UnexpectedWouldBlock;

    fn command_error(&self, error: AtError) -> Self::Error {
        StackError::ReceiveFailed(error)
    }
}

/// Command for receiving data
#[derive(Clone, AtatCmd)]
#[at_cmd("+CIPCLOSE", NoResponse, timeout_ms = 1_000)]
pub struct CloseSocketCommand {
    /// Socket ID
    link_id: usize,
}

impl CloseSocketCommand {
    pub fn new(link_id: usize) -> Self {
        Self { link_id }
    }
}

impl CommandErrorHandler for CloseSocketCommand {
    type Error = StackError;
    const WOULD_BLOCK_ERROR: Self::Error = StackError::UnexpectedWouldBlock;

    fn command_error(&self, error: AtError) -> Self::Error {
        StackError::CloseError(error)
    }
}

#[cfg(test)]
mod tests {
    use embedded_nal::{Ipv4Addr, Ipv6Addr};
    use heapless::String;

    use super::MAX_IP_LENGTH;

    macro_rules! test_v4 {
        ($a:expr, $b:expr, $c:expr, $d:expr, $string:literal) => {{
            assert_eq!(
                super::ipv4_to_string(&Ipv4Addr::new($a, $b, $c, $d)),
                String::<MAX_IP_LENGTH>::from($string)
            );
        }};
    }

    macro_rules! test_v6 {
        ($a:expr, $b:expr, $c:expr, $d:expr, $e:expr, $f:expr, $g:expr, $h:expr, $string:literal) => {{
            assert_eq!(
                super::ipv6_to_string(&Ipv6Addr::new($a, $b, $c, $d, $e, $f, $g, $h)),
                String::<MAX_IP_LENGTH>::from($string)
            );
        }};
    }

    #[test]
    fn test_ipv4_to_string() {
        test_v4!(127, 0, 0, 1, "127.0.0.1");
        test_v4!(0, 0, 0, 0, "0.0.0.0");
        test_v4!(255, 255, 255, 0, "255.255.255.0");
        test_v4!(255, 255, 255, 255, "255.255.255.255");
        test_v4!(1, 2, 3, 4, "1.2.3.4");
    }

    #[test]
    fn test_ipv6_to_string() {
        test_v6!(0, 0, 0, 0, 0, 0, 0, 1, "0:0:0:0:0:0:0:0001"); // ::1
        test_v6!(
            0x0102,
            0xaabb,
            0xffff,
            0x4242,
            0x0000,
            0x1111,
            0x2222,
            0x3333,
            "0102:aabb:ffff:4242:0:1111:2222:3333"
        );
    }
}
