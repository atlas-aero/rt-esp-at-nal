//! # WIFI access point client
//!
//! Joining a network and obtaining address information is supported.
//!
//! Note: If the connection was not successful or is lost, the ESP-AT will try independently fro time
//! to time (by default every second) to establish connection to the network. The status can be
//! queried using `get_join_state()`.
//!
//! ## Example
//!
//! ````
//! # use std::str::FromStr;
//! # use embedded_nal::{SocketAddr, TcpClientStack};
//! # use esp_at_nal::example::ExampleTimer;
//! # use esp_at_nal::wifi::{Adapter, WifiAdapter};
//! # use crate::esp_at_nal::example::ExampleAtClient as AtClient;
//! #
//! let client = AtClient::default();
//! let mut adapter: Adapter<_, _, 1_000_000, 256, 256> = Adapter::new(client, ExampleTimer::default());
//!
//! // Setting target WIFI access point
//! adapter.join("test_wifi", "secret").unwrap();
//!
//! // Waiting until a DCHP IP has been assigned
//! while !adapter.get_join_status().ip_assigned {}
//!
//! let address = adapter.get_address().unwrap();
//! assert_eq!("10:fe:ed:05:ba:50", address.mac.unwrap().as_str());
//! assert_eq!("10.0.0.181", address.ipv4.unwrap().to_string());
//! ````
use crate::commands::{
    AccessPointConnectCommand, CommandErrorHandler, ObtainLocalAddressCommand, SetSocketReceivingModeCommand,
    WifiModeCommand,
};
use crate::responses::LocalAddressResponse;
use crate::stack::SocketState;
use crate::urc::URCMessages;
use atat::heapless::Vec;
use atat::{AtatClient, AtatCmd, Error as AtError};
use core::fmt::Debug;
use core::str::FromStr;
use embedded_nal::{Ipv4Addr, Ipv6Addr};
use fugit::{ExtU32, TimerDurationU32};
use fugit_timer::Timer;
use heapless::String;

/// Wifi network adapter trait
pub trait WifiAdapter {
    /// Error when joining a WIFI network
    type JoinError: Debug;

    /// Error when receiving local address information
    type AddressError: Debug;

    /// Connects to an WIFI access point and returns the connection state
    fn join(&mut self, ssid: &str, key: &str) -> Result<JoinState, Self::JoinError>;

    /// Returns the current WIFI connection status
    fn get_join_status(&mut self) -> JoinState;

    /// Returns local address information
    fn get_address(&mut self) -> Result<LocalAddress, Self::AddressError>;
}

/// Central client for network communication
///
/// TX_SIZE: Chunk size in bytes when sending data. Higher value results in better performance, but
/// introduces also higher stack memory footprint. Max. value: 8192
///
/// RX_SIZE: Chunk size in bytes when receiving data. Value should be matched to buffer size of `receive()` calls.
pub struct Adapter<A: AtatClient, T: Timer<TIMER_HZ>, const TIMER_HZ: u32, const TX_SIZE: usize, const RX_SIZE: usize> {
    /// ATAT client
    pub(crate) client: A,

    /// Timer used for timeout measurement
    pub(crate) timer: T,

    /// Timeout for data transmission
    pub(crate) send_timeout: TimerDurationU32<TIMER_HZ>,

    /// Currently joined to WIFI network? Gets updated by URC messages.
    joined: bool,

    /// True if an IP was assigned by access point. Get updated by URC message.
    ip_assigned: bool,

    /// True if multiple connections have been enabled
    pub(crate) multi_connections_enabled: bool,

    /// True if socket passive receiving mode is enabled
    pub(crate) passive_mode_enabled: bool,

    /// Current socket states, array index = link_id
    pub(crate) sockets: [SocketState; 5],

    /// Data length available to receive which is buffered by ESP-AT. Array index = link_id
    pub(crate) data_available: [usize; 5],

    /// Received byte count confirmed by URC message. Gets reset to NONE by 'send()' method
    pub(crate) recv_byte_count: Option<usize>,

    /// True => Data transmission was confirmed by URC message
    /// False => Data transmission error signaled by URC message
    /// None => Neither an error or confirmed by received by URC message yet
    pub(crate) send_confirmed: Option<bool>,

    /// A URC message signaling that the given socket is already connected
    pub(crate) already_connected: bool,

    /// Received socket data by URC message
    pub(crate) data: Option<Vec<u8, RX_SIZE>>,
}

/// Possible errors when joining an access point
#[derive(Clone, Debug, PartialEq)]
pub enum JoinError {
    /// Error while setting the flash configuration mode
    ConfigurationStoreError(AtError),

    /// Error wile setting WIFI mode to station
    ModeError(AtError),

    /// Error while setting WIFI credentials
    ConnectError(AtError),

    /// Given SSD is longer then the max. size of 32 chars
    InvalidSSDLength,

    /// Given password is longer then the max. size of 63 chars
    InvalidPasswordLength,

    /// Received an unexpected WouldBlock. The most common cause of errors is an incorrect mode of the client.
    /// This must be either timeout or blocking.
    UnexpectedWouldBlock,
}

/// Errors when receiving local address information
#[derive(Clone, Debug, PartialEq)]
pub enum AddressErrors {
    /// CIFSR command failed
    CommandError(AtError),

    /// Error while parsing addresses
    AddressParseError,

    /// Received an unexpected WouldBlock. The most common cause of errors is an incorrect mode of the client.
    /// This must be either timeout or blocking.
    UnexpectedWouldBlock,
}

/// Current WIFI connection state
#[derive(Copy, Clone, Debug)]
pub struct JoinState {
    /// True if connected to an WIFI access point
    pub connected: bool,

    /// True if an IP was assigned
    pub ip_assigned: bool,
}

impl<A: AtatClient, T: Timer<TIMER_HZ>, const TIMER_HZ: u32, const TX_SIZE: usize, const RX_SIZE: usize> WifiAdapter
    for Adapter<A, T, TIMER_HZ, TX_SIZE, RX_SIZE>
{
    type JoinError = JoinError;
    type AddressError = AddressErrors;

    /// Connects to an WIFI access point and returns the connection state
    ///
    /// Note:
    /// If the connection was not successful or is lost, the ESP-AT will try independently fro time
    /// to time (by default every second) to establish connection to the network. The status can be
    /// queried using `get_join_state()`.
    fn join(&mut self, ssid: &str, key: &str) -> Result<JoinState, JoinError> {
        self.set_station_mode()?;
        self.connect_access_point(ssid, key)?;
        self.process_urc_messages();

        Ok(JoinState {
            connected: self.joined,
            ip_assigned: self.ip_assigned,
        })
    }

    /// Returns the current WIFI connection status
    fn get_join_status(&mut self) -> JoinState {
        self.process_urc_messages();
        JoinState {
            connected: self.joined,
            ip_assigned: self.ip_assigned,
        }
    }

    /// Returns local address information
    fn get_address(&mut self) -> Result<LocalAddress, AddressErrors> {
        let responses = self.send_command(ObtainLocalAddressCommand::new())?;
        LocalAddress::from_responses(responses)
    }
}

impl<A: AtatClient, T: Timer<TIMER_HZ>, const TIMER_HZ: u32, const TX_SIZE: usize, const RX_SIZE: usize>
    Adapter<A, T, TIMER_HZ, TX_SIZE, RX_SIZE>
{
    /// Creates a new network adapter. Client needs to be in timeout or blocking mode
    pub fn new(client: A, timer: T) -> Self {
        Self {
            client,
            timer,
            send_timeout: 5_000.millis(),
            joined: false,
            ip_assigned: false,
            multi_connections_enabled: false,
            passive_mode_enabled: false,
            sockets: [SocketState::Closed; 5],
            data_available: [0; 5],
            recv_byte_count: None,
            send_confirmed: None,
            already_connected: false,
            data: None,
        }
    }

    /// Processes all pending messages in the queue
    pub(crate) fn process_urc_messages(&mut self) {
        while let Some(message) = self.client.check_urc::<URCMessages<RX_SIZE>>() {
            self.handle_urc(message)
        }

        // Avoid full response queue, which gets full for a unknown reason
        let _ = self.client.check_response(&SetSocketReceivingModeCommand::passive_mode());
    }

    /// Handles a single URC message
    pub(crate) fn handle_urc(&mut self, message: URCMessages<RX_SIZE>) {
        match message {
            URCMessages::WifiDisconnected => {
                self.joined = false;
                self.ip_assigned = false;
            }
            URCMessages::ReceivedIP => self.ip_assigned = true,
            URCMessages::WifiConnected => self.joined = true,
            URCMessages::Ready => {}
            URCMessages::SocketConnected(link_id) => self.sockets[link_id] = SocketState::Connected,
            URCMessages::SocketClosed(link_id) => self.sockets[link_id] = SocketState::Closing,
            URCMessages::AlreadyConnected => self.already_connected = true,
            URCMessages::ReceivedBytes(count) => self.recv_byte_count = Some(count),
            URCMessages::SendConfirmation => self.send_confirmed = Some(true),
            URCMessages::SendFail => self.send_confirmed = Some(false),
            URCMessages::DataAvailable(link_id, length) => {
                if link_id < self.sockets.len() {
                    self.data_available[link_id] = length;
                }
            }
            URCMessages::Data(data) => self.data = Some(data),
            URCMessages::Echo => {}
            URCMessages::Unknown => {}
        }
    }

    /// Sends the command for switching to station mode
    fn set_station_mode(&mut self) -> Result<(), JoinError> {
        let command = WifiModeCommand::station_mode();
        self.send_command(command)?;

        Ok(())
    }

    /// Sends the command for setting the WIFI credentials
    fn connect_access_point(&mut self, ssid: &str, key: &str) -> Result<(), JoinError> {
        if ssid.len() > 32 {
            return Err(JoinError::InvalidSSDLength);
        }

        if key.len() > 63 {
            return Err(JoinError::InvalidPasswordLength);
        }

        let command = AccessPointConnectCommand::new(ssid.into(), key.into());
        self.send_command(command)?;

        Ok(())
    }

    /// Sends a command and maps the error if the command failed
    pub(crate) fn send_command<Cmd: AtatCmd<LEN> + CommandErrorHandler, const LEN: usize>(
        &mut self,
        command: Cmd,
    ) -> Result<Cmd::Response, Cmd::Error> {
        let result = self.client.send(&command);
        if let nb::Result::Err(error) = result {
            return match error {
                nb::Error::Other(other) => Err(command.command_error(other)),
                nb::Error::WouldBlock => Err(Cmd::WOULD_BLOCK_ERROR),
            };
        }

        Ok(result.unwrap())
    }

    /// Sets the timeout for sending TCP data in ms
    pub fn set_send_timeout_ms(&mut self, timeout: u32) {
        self.send_timeout = TimerDurationU32::millis(timeout);
    }
}

/// Local IP and MAC addresses
#[derive(Default, Clone, Debug)]
pub struct LocalAddress {
    /// Local IPv4 address if assigned
    pub ipv4: Option<Ipv4Addr>,

    /// Local MAC address
    pub mac: Option<String<17>>,

    /// Link local IPv6 address if assigned
    pub ipv6_link_local: Option<Ipv6Addr>,

    /// Global IPv6 address if assigned
    pub ipv6_global: Option<Ipv6Addr>,
}

impl LocalAddress {
    pub(crate) fn from_responses(responses: Vec<LocalAddressResponse, 4>) -> Result<Self, AddressErrors> {
        let mut data = Self::default();

        for response in responses {
            match response.address_type.as_slice() {
                b"STAIP" => {
                    data.ipv4 = Some(
                        Ipv4Addr::from_str(response.address.as_str()).map_err(|_| AddressErrors::AddressParseError)?,
                    )
                }
                b"STAIP6LL" => {
                    data.ipv6_link_local = Some(
                        Ipv6Addr::from_str(response.address.as_str()).map_err(|_| AddressErrors::AddressParseError)?,
                    )
                }
                b"STAIP6GL" => {
                    data.ipv6_global = Some(
                        Ipv6Addr::from_str(response.address.as_str()).map_err(|_| AddressErrors::AddressParseError)?,
                    )
                }
                b"STAMAC" => {
                    if response.address.len() > 17 {
                        return Err(AddressErrors::AddressParseError);
                    }

                    data.mac = Some(String::from(response.address.as_str()));
                }
                &_ => {}
            }
        }

        Ok(data)
    }
}
