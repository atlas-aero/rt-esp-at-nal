use crate::commands::{AccessPointConnectCommand, CommandErrorHandler, WifiModeCommand};
use crate::stack::SocketState;
use crate::urc::URCMessages;
use atat::{AtatClient, AtatCmd, Error as AtError};
use fugit::{ExtU32, TimerDurationU32};
use fugit_timer::Timer;

/// Central client for network communication
///
/// CHUNK_SIZE: Chunk size in bytes when sending data. Higher value results in better performance, but
/// introduces also higher stack memory footprint. Max. value: 8192
pub struct Adapter<A: AtatClient, T: Timer<TIMER_HZ>, const TIMER_HZ: u32, const CHUNK_SIZE: usize> {
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

    /// Received byte count confirmed by URC message. Gets reset to NONE by 'send()' method
    pub(crate) recv_byte_count: Option<usize>,

    /// True => Data transmission was confirmed by URC message
    /// False => Data transmission error signaled by URC message
    /// None => Neither an error or confirmed by received by URC message yet
    pub(crate) send_confirmed: Option<bool>,
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

/// Current WIFI connection state
#[derive(Copy, Clone, Debug)]
pub struct JoinState {
    /// True if connected to an WIFI access point
    pub connected: bool,

    /// True if an IP was assigned
    pub ip_assigned: bool,
}

impl<A: AtatClient, T: Timer<TIMER_HZ>, const TIMER_HZ: u32, const CHUNK_SIZE: usize>
    Adapter<A, T, TIMER_HZ, CHUNK_SIZE>
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
            recv_byte_count: None,
            send_confirmed: None,
        }
    }

    /// Connects to an WIFI access point and returns the connection state
    ///
    /// Note:
    /// If the connection was not successful or is lost, the ESP-AT will try independently fro time
    /// to time (by default every second) to establish connection to the network. The status can be
    /// queried using `get_join_state()`.
    pub fn join(&mut self, ssid: &str, key: &str) -> Result<JoinState, JoinError> {
        self.set_station_mode()?;
        self.connect_access_point(ssid, key)?;
        self.process_urc_messages();

        Ok(JoinState {
            connected: self.joined,
            ip_assigned: self.ip_assigned,
        })
    }

    /// Processes all pending messages in the queue
    pub fn process_urc_messages(&mut self) {
        while self.handle_single_urc() {}
    }

    /// Checks a single pending URC message. Returns false, if no URC message is pending
    fn handle_single_urc(&mut self) -> bool {
        match self.client.check_urc::<URCMessages>() {
            Some(URCMessages::WifiDisconnected) => {
                self.joined = false;
                self.ip_assigned = false;
            }
            Some(URCMessages::ReceivedIP) => self.ip_assigned = true,
            Some(URCMessages::WifiConnected) => self.joined = true,
            Some(URCMessages::Ready) => {}
            Some(URCMessages::SocketConnected(link_id)) => self.sockets[link_id] = SocketState::Connected,
            Some(URCMessages::SocketClosed(link_id)) => self.sockets[link_id] = SocketState::Closing,
            Some(URCMessages::ReceivedBytes(count)) => self.recv_byte_count = Some(count),
            Some(URCMessages::SendConfirmation) => self.send_confirmed = Some(true),
            Some(URCMessages::Error) => self.send_confirmed = Some(false),
            Some(URCMessages::Unknown) => {}
            None => return false,
        };

        true
    }

    /// Sends the command for switching to station mode
    fn set_station_mode(&mut self) -> Result<(), JoinError> {
        let command = WifiModeCommand::station_mode();
        self.send_command(command)
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
        self.send_command(command)
    }

    /// Sends a command and maps the error if the command failed
    pub(crate) fn send_command<Cmd: AtatCmd<LEN> + CommandErrorHandler, const LEN: usize>(
        &mut self,
        command: Cmd,
    ) -> Result<(), Cmd::Error> {
        if let nb::Result::Err(error) = self.client.send(&command) {
            return match error {
                nb::Error::Other(other) => Err(command.command_error(other)),
                nb::Error::WouldBlock => Err(Cmd::WOULD_BLOCK_ERROR),
            };
        }

        Ok(())
    }

    /// Sets the timeout for sending TCP data in ms
    pub fn set_send_timeout_ms(&mut self, timeout: u32) {
        self.send_timeout = TimerDurationU32::millis(timeout);
    }
}
