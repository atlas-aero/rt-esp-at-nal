use core::future::poll_fn;
use core::net::SocketAddr;
use core::{cell::RefCell, fmt::Debug};

use core::str::FromStr;
use embassy_futures::select::Either;
use embassy_futures::yield_now;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::mutex::Mutex;
use embassy_time::{Duration, Timer};
use embedded_nal_async::TcpConnect;
use heapless::String;
use atat::{asynch::AtatClient, AtatCmd, UrcSubscription};

use crate::commands::{ConnectCommand, SetMultipleConnectionsCommand, SetSocketReceivingModeCommand};
use crate::stack::{ConnectionState, Error, Socket};
use crate::{commands::{AccessPointConnectCommand, AutoConnectCommand, CommandErrorHandler, ObtainLocalAddressCommand, RestartCommand, WifiModeCommand}, urc::URCMessages, wifi::{AddressErrors, CommandError, JoinError, JoinState, LocalAddress, Session}};

use super::connection::Connection;

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
    fn join(&mut self, ssid: &str, key: &str) -> impl core::future::Future<Output = Result<JoinState, Self::JoinError>>;

    /// Returns the current WIFI connection status
    fn get_join_status(&mut self) -> impl core::future::Future<Output = JoinState>;

    /// Returns local address information
    fn get_address(&mut self) -> impl core::future::Future<Output = Result<LocalAddress, Self::AddressError>>;

    /// Enables/Disables auto connect, so that ESP-AT whether automatically joins to the stored AP when powered on.
    fn set_auto_connect(&mut self, enabled: bool) -> impl core::future::Future<Output = Result<(), Self::ConfigurationErrors>>;

    /// Restarts the module and blocks until ready
    fn restart(&mut self) -> impl core::future::Future<Output = Result<(), Self::RestartError>>;
}

pub struct InnerAdapter<
    'urc_sub,
    A: AtatClient,
    const TX_SIZE: usize,
    const RX_SIZE: usize,
    const URC_CAPACITY: usize,
> {
    /// ATAT client
    pub(crate) client: A,

    /// URC message subscriber
    pub(crate) urc_subscription: UrcSubscription<'urc_sub, URCMessages<RX_SIZE>, URC_CAPACITY, 1>,

    /// Network state
    pub(crate) session: Session<RX_SIZE>,

    /// Timeout for data transmission
    pub(crate) send_timeout: Duration,
}

impl<
        'urc_sub,
        A: AtatClient,
        const TX_SIZE: usize,
        const RX_SIZE: usize,
        const URC_CAPACITY: usize,
    > InnerAdapter<'urc_sub, A, TX_SIZE, RX_SIZE, URC_CAPACITY>
{
    pub fn new(
        client: A,
        urc_subscription: UrcSubscription<'urc_sub, URCMessages<RX_SIZE>, URC_CAPACITY, 1>,
        send_timeout: Duration,
    ) -> Self {
        Self {
            client,
            urc_subscription,
            session: Session::default(),
            send_timeout,
        }
    }

    pub(crate) fn process_urc_messages(&mut self) {
        while let Some(message) = self.urc_subscription.try_next_message_pure() {
            self.session.handle_urc(message)
        }
    }

    async fn set_station_mode(&mut self) -> Result<(), JoinError> {
        let command = WifiModeCommand::station_mode();
        self.send_command(command).await?;

        Ok(())
    }

    async fn connect_access_point(&mut self, ssid: &str, key: &str) -> Result<(), JoinError> {
        if ssid.len() > 32 {
            return Err(JoinError::InvalidSSDLength);
        }

        if key.len() > 63 {
            return Err(JoinError::InvalidPasswordLength);
        }

        let command = AccessPointConnectCommand::new(String::from_str(ssid).unwrap(), String::from_str(key).unwrap());
        self.send_command(command).await?;

        Ok(())
    }

    pub(crate) async fn send_command<Cmd: AtatCmd + CommandErrorHandler>(
        &mut self,
        command: Cmd,
    ) -> Result<Cmd::Response, Cmd::Error> {
        self.client.send(&command).await.map_err(|e| command.command_error(e))
    }

    async fn enable_multiple_connections(&mut self) -> Result<(), Error> {
        if self.session.multi_connections_enabled {
            return Ok(());
        }

        self.send_command(SetMultipleConnectionsCommand::multiple()).await?;
        self.session.multi_connections_enabled = true;
        Ok(())
    }

    fn open_socket(&mut self) -> Result<Socket, Error> {
        if let Some(link_id) = self.session.get_next_open() {
            self.session.sockets[link_id].state = ConnectionState::Open;
            return Ok(Socket::new(link_id));
        }

        Err(Error::NoSocketAvailable)
    }

    async fn enable_passive_receiving_mode(&mut self) -> Result<(), Error> {
        if self.session.passive_mode_enabled {
            return Ok(());
        }

        self.send_command(SetSocketReceivingModeCommand::passive_mode()).await?;
        self.session.passive_mode_enabled = true;
        Ok(())
    }
}

pub struct Adapter<
    'urc_sub,
    A: AtatClient,
    const TX_SIZE: usize,
    const RX_SIZE: usize,
    const URC_CAPACITY: usize,
> {
    inner: Mutex<CriticalSectionRawMutex, InnerAdapter<'urc_sub, A, TX_SIZE, RX_SIZE, URC_CAPACITY>>,
}

impl<
        A: AtatClient,
        const TX_SIZE: usize,
        const RX_SIZE: usize,
        const URC_CAPACITY: usize,
    > WifiAdapter for Adapter<'_, A, TX_SIZE, RX_SIZE, URC_CAPACITY>
{
    type JoinError = JoinError;
    type AddressError = AddressErrors;
    type ConfigurationErrors = CommandError;
    type RestartError = CommandError;

    async fn join(&mut self, ssid: &str, key: &str) -> Result<JoinState, JoinError> {
        let mut inner = self.inner.lock().await;
        inner.set_station_mode().await?;
        inner.connect_access_point(ssid, key).await?;
        inner.process_urc_messages();

        Ok(JoinState {
            connected: inner.session.joined,
            ip_assigned: inner.session.ip_assigned,
        })
    }

    async fn get_join_status(&mut self) -> JoinState {
        let mut inner = self.inner.lock().await;
        inner.process_urc_messages();
        JoinState {
            connected: inner.session.joined,
            ip_assigned: inner.session.ip_assigned,
        }
    }

    async fn get_address(&mut self) -> Result<LocalAddress, AddressErrors> {
        let responses = self.inner.lock().await.send_command(ObtainLocalAddressCommand::new()).await?;
        LocalAddress::from_responses(responses)
    }

    async fn set_auto_connect(&mut self, enabled: bool) -> Result<(), CommandError> {
        self.inner.lock().await.send_command(AutoConnectCommand::new(enabled)).await?;
        Ok(())
    }

    async fn restart(&mut self) -> Result<(), CommandError> {
        let mut inner = self.inner.lock().await;
        inner.session.ready = false;
        inner.send_command(RestartCommand::default()).await?;

        inner.session = Session::default();

        let task = async {
            while !inner.session.ready {
                inner.process_urc_messages();
                yield_now().await;
            }
        };

        let res = embassy_futures::select::select(
            Timer::after_secs(5),
            task,
        ).await;

        match res {
            Either::First(_) => Err(CommandError::ReadyTimeout),
            Either::Second(_) => Ok(())
        }
    }
}

impl<
        'urc_sub,
        A: AtatClient,
        const TX_SIZE: usize,
        const RX_SIZE: usize,
        const URC_CAPACITY: usize,
    > Adapter<'urc_sub, A, TX_SIZE, RX_SIZE, URC_CAPACITY>
{
    pub fn new(
        client: A,
        urc_subscription: UrcSubscription<'urc_sub, URCMessages<RX_SIZE>, URC_CAPACITY, 1>,
        send_timeout: Duration,
    ) -> Self {
        Self {
            inner: Mutex::new(InnerAdapter::new(client, urc_subscription, send_timeout)),
        }
    }
}

impl<
    'urc_sub,
    A: AtatClient,
    const TX_SIZE: usize,
    const RX_SIZE: usize,
    const URC_CAPACITY: usize,
> TcpConnect for Adapter<'urc_sub, A, TX_SIZE, RX_SIZE, URC_CAPACITY>
{
    type Error = Error;

    type Connection<'a> = Connection<'a, 'urc_sub, A, TX_SIZE, RX_SIZE, URC_CAPACITY>
        where
            Self: 'a;

    async fn connect<'a>(&'a self, remote: SocketAddr) -> Result<Self::Connection<'a>, Self::Error> {
        let mut inner = self.inner.lock().await;
        inner.enable_multiple_connections().await?;
        let socket = inner.open_socket()?;
        inner.process_urc_messages();

        if inner.session.is_socket_connected(&socket) {
            return Err(Error::AlreadyConnected);
        }

        inner.enable_passive_receiving_mode().await?;
        inner.session.already_connected = false;

        let command = match remote {
            SocketAddr::V4(address) => ConnectCommand::tcp_v4(socket.link_id, address),
            SocketAddr::V6(address) => ConnectCommand::tcp_v6(socket.link_id, address),
        };
        let result = inner.send_command(command).await;
        inner.process_urc_messages();

        // ESP-AT returned that given socket is already connected. This indicates that a URC Connect message was missed.
        if inner.session.already_connected {
            inner.session.sockets[socket.link_id].state = ConnectionState::Connected;
            return Ok(Connection {
                socket,
                inner: &self.inner,
            });
        }
        result?;

        if !inner.session.is_socket_connected(&socket) {
            return Err(Error::UnconfirmedSocketState);
        }

        inner.session.reset_available_data(&socket);
        Ok(Connection {
            socket,
            inner: &self.inner,
        })
    }
}
