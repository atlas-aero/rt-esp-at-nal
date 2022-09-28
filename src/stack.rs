use crate::adapter::Adapter;
use crate::commands::{
    ConnectCommand, SetMultipleConnectionsCommand, SetSocketReceivingModeCommand, TransmissionCommand,
    TransmissionPrepareCommand,
};
use atat::AtatClient;
use atat::Error as AtError;
use embedded_nal::{SocketAddr, TcpClientStack};
use fugit_timer::Timer;

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
    Connected,
    /// Socket was closed by URC message, but Socket object still exists and needs to be fully closed by calling 'close()'
    Closing,
}

/// Network related errors
#[derive(Clone, Debug, PartialEq)]
pub enum Error {
    /// Error while sending CIPMUX command for enabling multiple connections
    EnablingMultiConnectionsFailed(AtError),

    /// Error while sending CIPRECVMODE command for enabling passive socket receiving mode
    EnablingPassiveSocketModeFailed(AtError),

    /// TCP connect command failed
    ConnectError(AtError),

    /// Preparing the transmission failed (CIPSEND command)
    TransmissionStartFailed(AtError),

    /// Transmission of data failed
    SendFailed(AtError),

    /// AT-ESP confirmed receiving an unexpected byte count
    PartialSend,

    /// TCP connect command was responded by by OK. But connect was not confirmed by URC message.
    ConnectUnconfirmed,

    /// No socket available, since the maximum number is in use.
    NoSocketAvailable,

    /// Given socket is already connected to another remote. Socket needs to be closed first.
    AlreadyConnected,

    /// Unable to send data if socket is not connected
    SocketUnconnected,

    /// Socket was remotely closed and needs to either reconnected to fully closed by calling `close()` for [Adapter]
    ClosingSocket,

    /// Received an unexpected WouldBlock. The most common cause of errors is an incorrect mode of the client.
    /// This must be either timeout or blocking.
    UnexpectedWouldBlock,

    /// Upstream timer error
    TimerError,
}

impl<A: AtatClient, T: Timer<TIMER_HZ>, const TIMER_HZ: u32, const CHUNK_SIZE: usize> TcpClientStack
    for Adapter<A, T, TIMER_HZ, CHUNK_SIZE>
{
    type TcpSocket = Socket;
    type Error = Error;

    fn socket(&mut self) -> Result<Self::TcpSocket, Self::Error> {
        self.enable_multiple_connections()?;
        self.open_socket()
    }

    fn connect(&mut self, socket: &mut Socket, remote: SocketAddr) -> nb::Result<(), Self::Error> {
        self.process_urc_messages();

        if self.sockets[socket.link_id] == SocketState::Connected {
            return nb::Result::Err(nb::Error::Other(Error::AlreadyConnected));
        }

        self.enable_passive_receiving_mode()?;

        let command = match remote {
            SocketAddr::V4(address) => ConnectCommand::tcp_v4(socket.link_id, address),
            SocketAddr::V6(address) => ConnectCommand::tcp_v6(socket.link_id, address),
        };

        self.send_command(command)?;
        self.process_urc_messages();

        if self.sockets[socket.link_id] != SocketState::Connected {
            return nb::Result::Err(nb::Error::Other(Error::ConnectUnconfirmed));
        }

        nb::Result::Ok(())
    }

    fn is_connected(&mut self, _socket: &Self::TcpSocket) -> Result<bool, Self::Error> {
        todo!()
    }

    fn send(&mut self, socket: &mut Socket, buffer: &[u8]) -> nb::Result<usize, Error> {
        self.process_urc_messages();
        self.assert_socket_connected(socket)?;

        for chunk in buffer.chunks(CHUNK_SIZE) {
            self.send_command(TransmissionPrepareCommand::new(socket.link_id, chunk.len()))?;
            self.send_chunk(chunk)?;
        }

        nb::Result::Ok(buffer.len())
    }

    fn receive(&mut self, _socket: &mut Self::TcpSocket, _buffer: &mut [u8]) -> nb::Result<usize, Self::Error> {
        todo!()
    }

    fn close(&mut self, _socket: Self::TcpSocket) -> Result<(), Self::Error> {
        todo!()
    }
}

impl<A: AtatClient, T: Timer<TIMER_HZ>, const TIMER_HZ: u32, const CHUNK_SIZE: usize>
    Adapter<A, T, TIMER_HZ, CHUNK_SIZE>
{
    /// Sends a chunk of max. 256 bytes
    fn send_chunk(&mut self, data: &[u8]) -> Result<(), Error> {
        self.send_confirmed = None;
        self.recv_byte_count = None;

        self.send_command::<TransmissionCommand<'_>, CHUNK_SIZE>(TransmissionCommand::new(data))?;
        self.timer.start(self.send_timeout).map_err(|_| Error::TimerError)?;

        while self.send_confirmed.is_none() {
            self.process_urc_messages();

            if let Some(send_success) = self.send_confirmed {
                // Transmission failed
                if !send_success {
                    return Err(Error::SendFailed(AtError::Error));
                }

                // Byte count does not match
                if self.recv_byte_count.is_some() && *self.recv_byte_count.as_ref().unwrap() != data.len() {
                    return Err(Error::PartialSend);
                }

                return Ok(());
            }

            match self.timer.wait() {
                Ok(_) => return Err(Error::SendFailed(AtError::Timeout)),
                Err(error) => match error {
                    nb::Error::Other(_) => return Err(Error::TimerError),
                    nb::Error::WouldBlock => {}
                },
            }
        }

        Ok(())
    }

    /// Enables multiple connections.
    /// Stores internal state, so command is just sent once for saving bandwidth
    fn enable_multiple_connections(&mut self) -> Result<(), Error> {
        if self.multi_connections_enabled {
            return Ok(());
        }

        self.send_command(SetMultipleConnectionsCommand::multiple())?;
        self.multi_connections_enabled = true;
        Ok(())
    }

    /// Enables the passive socket receiving mode
    /// Stores internal state, so command is just sent once for saving bandwidth
    fn enable_passive_receiving_mode(&mut self) -> Result<(), Error> {
        if self.passive_mode_enabled {
            return Ok(());
        }

        self.send_command(SetSocketReceivingModeCommand::passive_mode())?;
        self.passive_mode_enabled = true;
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

    /// Asserts that the given socket is connected and returns otherwise the appropriate error
    fn assert_socket_connected(&self, socket: &Socket) -> nb::Result<(), Error> {
        if self.sockets[socket.link_id] == SocketState::Closing {
            return nb::Result::Err(nb::Error::Other(Error::ClosingSocket));
        }

        if self.sockets[socket.link_id] != SocketState::Connected {
            return nb::Result::Err(nb::Error::Other(Error::SocketUnconnected));
        }

        nb::Result::Ok(())
    }
}
