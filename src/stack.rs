use crate::adapter::Adapter;
use crate::commands::SetMultipleConnectionsCommand;
use atat::AtatClient;
use atat::Error as AtError;
use embedded_nal::{SocketAddr, TcpClientStack};

/// Unique socket for a network connection
#[derive(Debug)]
pub struct Socket {
    /// Unique link id of AT
    #[allow(unused)]
    pub(crate) link_id: usize,
}

impl Socket {
    pub(crate) fn new(link_id: usize) -> Self {
        Self { link_id }
    }
}

/// Internal connection state
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub(crate) enum SocketState {
    /// Socket is closed an may be (re)used
    Closed,
    /// Socket was returned by socket() but is not connected yet
    Open,
    /// Connection is fully open
    #[allow(unused)]
    Connected,
}

/// Network related errors
#[derive(Clone, Debug, PartialEq)]
pub enum Error {
    /// Error while sending CIPMUX command for enabling multiple connections
    EnablingMultiConnectionsFailed(AtError),

    /// No socket available, since the maximum number is in use.
    NoSocketAvailable,

    /// Received an unexpected WouldBlock. The most common cause of errors is an incorrect mode of the client.
    /// This must be either timeout or blocking.
    UnexpectedWouldBlock,
}

impl<A: AtatClient> TcpClientStack for Adapter<A> {
    type TcpSocket = Socket;
    type Error = Error;

    fn socket(&mut self) -> Result<Self::TcpSocket, Self::Error> {
        self.enable_multiple_connections()?;
        self.open_socket()
    }

    fn connect(&mut self, _socket: &mut Self::TcpSocket, _remote: SocketAddr) -> nb::Result<(), Self::Error> {
        todo!()
    }

    fn is_connected(&mut self, _socket: &Self::TcpSocket) -> Result<bool, Self::Error> {
        todo!()
    }

    fn send(&mut self, _socket: &mut Self::TcpSocket, _buffer: &[u8]) -> nb::Result<usize, Self::Error> {
        todo!()
    }

    fn receive(&mut self, _socket: &mut Self::TcpSocket, _buffer: &mut [u8]) -> nb::Result<usize, Self::Error> {
        todo!()
    }

    fn close(&mut self, _socket: Self::TcpSocket) -> Result<(), Self::Error> {
        todo!()
    }
}

impl<A: AtatClient> Adapter<A> {
    /// Enables multiple connections.
    /// Stores internal state, so command is just sent once for saving bandwidth
    fn enable_multiple_connections(&mut self) -> Result<(), Error> {
        if self.multi_connections_enabled {
            return Ok(());
        }

        let command = SetMultipleConnectionsCommand::multiple();
        if let nb::Result::Err(error) = self.client.send(&command) {
            return match error {
                nb::Error::Other(other) => Err(Error::EnablingMultiConnectionsFailed(other)),
                nb::Error::WouldBlock => Err(Error::UnexpectedWouldBlock),
            };
        }

        self.multi_connections_enabled = true;
        Ok(())
    }

    /// Assigns a free link_id. Returns an error in case no more free sockets are available
    fn open_socket(&mut self) -> Result<Socket, Error> {
        if let Some(link_id) = self.sockets.iter().position(|state| state == &SocketState::Closed) {
            self.sockets[link_id] = SocketState::Open;
            return Ok(Socket::new(link_id));
        }

        Err(Error::NoSocketAvailable)
    }
}
