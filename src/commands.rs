use crate::adapter::{AddressErrors, JoinError};
use crate::responses::LocalAddressResponse;
use crate::responses::NoResponse;
use crate::stack::Error as StackError;
use alloc::string::ToString;
use atat::atat_derive::AtatCmd;
use atat::heapless::{String, Vec};
use atat::{AtatCmd, Error as AtError, InternalError};
use embedded_nal::{SocketAddrV4, SocketAddrV6};

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
