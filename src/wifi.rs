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
//! # use core::str::FromStr;
//! # use embedded_nal::{TcpClientStack};
//! # use esp_at_nal::example::ExampleTimer;
//! # use esp_at_nal::wifi::{Adapter, WifiAdapter};
//! # use crate::esp_at_nal::example::ExampleAtClient as AtClient;
//! #
//! let urc_channel = AtClient::urc_channel();
//! let client = AtClient::init(&urc_channel);
//! let mut adapter: Adapter<_, _, 1_000_000, 1024, 128, 8> = Adapter::new(client, urc_channel.subscriber().unwrap(), ExampleTimer::default());
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
    AccessPointConnectCommand, AutoConnectCommand, CommandErrorHandler, ObtainLocalAddressCommand, RestartCommand,
    WifiModeCommand,
};
use crate::responses::LocalAddressResponse;
use crate::stack::{ConnectionState, SocketState};
use crate::urc::URCMessages;
use atat::blocking::AtatClient;
use atat::heapless::Vec;
use atat::{AtatCmd, Error as AtError, UrcSubscription};
use core::fmt::Debug;
use core::net::{Ipv4Addr, Ipv6Addr};
use core::str::FromStr;
use fugit::{ExtU32, TimerDurationU32};
use fugit_timer::Timer;
use heapless::String;
use nb::Error;

/// Wifi network adapter trait
pub trait WifiAdapter {
    /// Error when joining a WIFI network
    type JoinError: Debug;

    /// Error when receiving local address information
    type AddressError: Debug;

    /// Errors for configuration commands
    type ConfigurationErrors: Debug;

    /// Errors when restarting the module
    type RestartError: Debug;

    /// Connects to an WIFI access point and returns the connection state
    fn join(&mut self, ssid: &str, key: &str) -> Result<JoinState, Self::JoinError>;

    /// Returns the current WIFI connection status
    fn get_join_status(&mut self) -> JoinState;

    /// Returns local address information
    fn get_address(&mut self) -> Result<LocalAddress, Self::AddressError>;

    /// Enables/Disables auto connect, so that ESP-AT whether automatically joins to the stored AP when powered on.
    fn set_auto_connect(&mut self, enabled: bool) -> Result<(), Self::ConfigurationErrors>;

    /// Restarts the module and blocks until ready
    fn restart(&mut self) -> Result<(), Self::RestartError>;
}

/// Central client for network communication
///
/// TX_SIZE: Chunk size in bytes when sending data. Higher value results in better performance, but
/// introduces also higher stack memory footprint. Max. value: 8192
///
/// RX_SIZE: Chunk size in bytes when receiving data. Value should be matched to buffer size of `receive()` calls.
///
/// URC_CAPACITY: URC buffer size. It's the same value, as used when initializing the UrcChannel of atat
pub struct Adapter<
    'urc_sub,
    A: AtatClient,
    T: Timer<TIMER_HZ>,
    const TIMER_HZ: u32,
    const TX_SIZE: usize,
    const RX_SIZE: usize,
    const URC_CAPACITY: usize,
> {
    /// ATAT client
    pub(crate) client: A,

    /// URC message subscriber
    pub(crate) urc_subscription: UrcSubscription<'urc_sub, URCMessages<RX_SIZE>, URC_CAPACITY, 1>,

    /// Timer used for timeout measurement
    pub(crate) timer: T,

    /// Timeout for data transmission
    pub(crate) send_timeout: TimerDurationU32<TIMER_HZ>,

    /// Network state
    pub(crate) session: Session<RX_SIZE>,
}

/// Collection of network state
#[derive(Default)]
pub(crate) struct Session<const RX_SIZE: usize> {
    /// Currently joined to WIFI network? Gets updated by URC messages.
    pub(crate) joined: bool,

    /// True if an IP was assigned by access point. Get updated by URC message.
    pub(crate) ip_assigned: bool,

    /// True if a URC ready message arrived.
    pub(crate) ready: bool,

    /// True if multiple connections have been enabled
    pub(crate) multi_connections_enabled: bool,

    /// True if socket passive receiving mode is enabled
    pub(crate) passive_mode_enabled: bool,

    /// Current socket states, array index = link_id
    pub(crate) sockets: [SocketState; 5],

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

impl<const RX_SIZE: usize> Session<RX_SIZE> {
    /// Handles a single URC message
    pub(crate) fn handle_urc(&mut self, message: URCMessages<RX_SIZE>) {
        match message {
            URCMessages::WifiDisconnected => {
                self.joined = false;
                self.ip_assigned = false;
            }
            URCMessages::ReceivedIP => self.ip_assigned = true,
            URCMessages::WifiConnected => self.joined = true,
            URCMessages::Ready => self.ready = true,
            URCMessages::SocketConnected(link_id) => self.sockets[link_id].state = ConnectionState::Connected,
            URCMessages::SocketClosed(link_id) => self.sockets[link_id].state = ConnectionState::Closing,
            URCMessages::AlreadyConnected => self.already_connected = true,
            URCMessages::ReceivedBytes(count) => self.recv_byte_count = Some(count),
            URCMessages::SendConfirmation => self.send_confirmed = Some(true),
            URCMessages::SendFail => self.send_confirmed = Some(false),
            URCMessages::DataAvailable(link_id, length) => {
                if link_id < self.sockets.len() {
                    self.sockets[link_id].data_available = Some(length);
                }
            }
            URCMessages::Data(data) => self.data = Some(data),
            URCMessages::Unknown => {}
        }
    }
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

/// General errors for simple commands (e.g. enabling a configuration flag)
#[derive(Clone, Debug, PartialEq)]
pub enum CommandError {
    /// Command failed with the given upstream error
    CommandFailed(AtError),

    /// No ready message received within timout (5 seconds)
    ReadyTimeout,

    /// Upstream timer error
    TimerError,

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

impl<
        A: AtatClient,
        T: Timer<TIMER_HZ>,
        const TIMER_HZ: u32,
        const TX_SIZE: usize,
        const RX_SIZE: usize,
        const URC_CAPACITY: usize,
    > WifiAdapter for Adapter<'_, A, T, TIMER_HZ, TX_SIZE, RX_SIZE, URC_CAPACITY>
{
    type JoinError = JoinError;
    type AddressError = AddressErrors;
    type ConfigurationErrors = CommandError;
    type RestartError = CommandError;

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
            connected: self.session.joined,
            ip_assigned: self.session.ip_assigned,
        })
    }

    /// Returns the current WIFI connection status
    fn get_join_status(&mut self) -> JoinState {
        self.process_urc_messages();
        JoinState {
            connected: self.session.joined,
            ip_assigned: self.session.ip_assigned,
        }
    }

    /// Returns local address information
    fn get_address(&mut self) -> Result<LocalAddress, AddressErrors> {
        let responses = self.send_command(ObtainLocalAddressCommand::new())?;
        LocalAddress::from_responses(responses)
    }

    /// Enables auto connect, so that ESP-AT automatically connects to the stored AP when powered on.
    fn set_auto_connect(&mut self, enabled: bool) -> Result<(), CommandError> {
        self.send_command(AutoConnectCommand::new(enabled))?;
        Ok(())
    }

    /// Restarts the module and blocks until the module is ready.
    /// If module is not ready within five seconds, [CommandError::ReadyTimeout] is returned
    fn restart(&mut self) -> Result<(), CommandError> {
        self.session.ready = false;
        self.send_command(RestartCommand::default())?;

        self.session = Session::default();

        self.timer.start(5.secs()).map_err(|_| CommandError::TimerError)?;
        while !self.session.ready {
            if let nb::Result::Err(error) = self.timer.wait() {
                match error {
                    Error::Other(_) => return Err(CommandError::TimerError),
                    Error::WouldBlock => {}
                }
            } else {
                return Err(CommandError::ReadyTimeout);
            }

            self.process_urc_messages();
        }

        Ok(())
    }
}

impl<
        'urc_sub,
        A: AtatClient,
        T: Timer<TIMER_HZ>,
        const TIMER_HZ: u32,
        const TX_SIZE: usize,
        const RX_SIZE: usize,
        const URC_CAPACITY: usize,
    > Adapter<'urc_sub, A, T, TIMER_HZ, TX_SIZE, RX_SIZE, URC_CAPACITY>
{
    /// Creates a new network adapter. Client needs to be in timeout or blocking mode
    pub fn new(
        client: A,
        urc_subscription: UrcSubscription<'urc_sub, URCMessages<RX_SIZE>, URC_CAPACITY, 1>,
        timer: T,
    ) -> Self {
        Self {
            client,
            urc_subscription,
            timer,
            send_timeout: 5_000.millis(),
            session: Session::default(),
        }
    }

    /// Processes all pending messages in the queue
    pub(crate) fn process_urc_messages(&mut self) {
        while let Some(message) = self.urc_subscription.try_next_message_pure() {
            self.session.handle_urc(message)
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

        let command = AccessPointConnectCommand::new(String::from_str(ssid).unwrap(), String::from_str(key).unwrap());
        self.send_command(command)?;

        Ok(())
    }

    /// Sends a command and maps the error if the command failed
    pub(crate) fn send_command<Cmd: AtatCmd + CommandErrorHandler>(
        &mut self,
        command: Cmd,
    ) -> Result<Cmd::Response, Cmd::Error> {
        self.client.send(&command).map_err(|e| command.command_error(e))
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

                    data.mac = match String::from_str(response.address.as_str()) {
                        Ok(string) => Some(string),
                        Err(_) => None,
                    };
                }
                &_ => {}
            }
        }

        Ok(data)
    }
}
