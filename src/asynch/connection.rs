use atat::asynch::AtatClient;
use atat::Error as AtError;
use embassy_futures::{select::{select, Either}, yield_now};
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::mutex::Mutex;
use embassy_time::Timer;

use crate::{commands::{ReceiveDataCommand, TransmissionCommand, TransmissionPrepareCommand}, stack::{Buffer, Error, Socket}};

use super::wifi::InnerAdapter;

impl embedded_io::Error for Error {
    fn kind(&self) -> embedded_io::ErrorKind {
        match self {
            _ => embedded_io::ErrorKind::Other,
        }
    }
}

pub struct Connection<
    'inner,
    'urc_sub,
    A: AtatClient,
    const TX_SIZE: usize,
    const RX_SIZE: usize,
    const URC_CAPACITY: usize,
> {
    pub(crate) socket: Socket,
    pub(crate) inner: &'inner Mutex<CriticalSectionRawMutex, InnerAdapter<'urc_sub, A, TX_SIZE, RX_SIZE, URC_CAPACITY>>,
}

impl<
    'inner,
    'urc_sub,
    A: AtatClient,
    const TX_SIZE: usize,
    const RX_SIZE: usize,
    const URC_CAPACITY: usize,
> embedded_io::ErrorType for Connection<'inner, 'urc_sub, A, TX_SIZE, RX_SIZE, URC_CAPACITY>
{
    type Error = Error;
}

impl<
    'inner,
    'urc_sub,
    A: AtatClient,
    const TX_SIZE: usize,
    const RX_SIZE: usize,
    const URC_CAPACITY: usize,
> embedded_io_async::Read for Connection<'inner, 'urc_sub, A, TX_SIZE, RX_SIZE, URC_CAPACITY>
{
    async fn read(&mut self, buf: &mut [u8]) -> Result<usize, Self::Error> {
        let mut inner = self.inner.lock().await;
        inner.process_urc_messages();

        while !inner.session.is_data_available(&self.socket) {
            inner.process_urc_messages();
            yield_now().await;
        }
        
        let mut buffer: Buffer<RX_SIZE> = Buffer::new(buf);

        loop {
            inner.session.take_data_available(&self.socket);

            let command = ReceiveDataCommand::<RX_SIZE>::new(self.socket.link_id, buffer.get_next_length());
            inner.send_command(command).await?;
            inner.process_urc_messages();

            if inner.session.data.is_none() {
                return Err(Error::ReceiveFailed(AtError::InvalidResponse));
            }

            let data = inner.session.data.take().unwrap();
            buffer.append(data)?;

            if !inner.session.is_data_available(&self.socket) || buffer.is_full() {
                return Ok(buffer.len());
            }
        }
    }
}

impl<
    'inner,
    'urc_sub,
    A: AtatClient,
    const TX_SIZE: usize,
    const RX_SIZE: usize,
    const URC_CAPACITY: usize,
> embedded_io_async::Write for Connection<'inner, 'urc_sub, A, TX_SIZE, RX_SIZE, URC_CAPACITY>
{
    async fn write(&mut self, buf: &[u8]) -> Result<usize, Self::Error> {
        let mut inner = self.inner.lock().await;
        inner.process_urc_messages();
        inner.assert_socket_connected(&self.socket)?;

        for chunk in buf.chunks(TX_SIZE) {
            inner.send_command(TransmissionPrepareCommand::new(self.socket.link_id, chunk.len())).await?;
            inner.send_chunk(chunk).await?;
        }

        Ok(buf.len())
    }
}

impl<
    'urc_sub,
    A: AtatClient,
    const TX_SIZE: usize,
    const RX_SIZE: usize,
    const URC_CAPACITY: usize,
> InnerAdapter<'urc_sub, A, TX_SIZE, RX_SIZE, URC_CAPACITY>
{
    fn assert_socket_connected(&self, socket: &Socket) -> Result<(), Error> {
        if self.session.is_socket_closing(socket) {
            return Err(Error::ClosingSocket);
        }

        if !self.session.is_socket_connected(socket) {
            return Err(Error::SocketUnconnected);
        }

        Ok(())
    }

    async fn send_chunk(&mut self, data: &[u8]) -> Result<(), Error> {
        self.session.send_confirmed = None;
        self.session.recv_byte_count = None;

        self.send_command::<TransmissionCommand<'_, TX_SIZE>>(TransmissionCommand::new(data)).await?;
        let timer = Timer::after(self.send_timeout);

        let task = async {
            while self.session.send_confirmed.is_none() {
                self.process_urc_messages();

                if let Some(send_success) = self.session.send_confirmed {
                    // Transmission failed
                    if !send_success {
                        return Err(Error::SendFailed(AtError::Error));
                    }

                    // Byte count does not match
                    if self.session.is_received_byte_count_incorrect(data.len()) {
                        return Err(Error::PartialSend);
                    }

                    return Ok(());
                }

                yield_now().await;
            }

            Ok(())
        };

        match select(timer, task).await {
            Either::First(_) => Err(Error::SendFailed(AtError::Timeout)),
            Either::Second(res) => res,
        }
    }
}
