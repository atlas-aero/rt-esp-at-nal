use crate::urc::URCMessages;
use alloc::collections::VecDeque;
use atat::blocking::AtatClient;
use atat::{AtatCmd, AtatUrc, Error};
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::pubsub::{PubSubChannel, Publisher};
use fugit::{TimerDurationU32, TimerInstantU32};
use fugit_timer::Timer as FugitTimer;
use mockall::mock;

/// Custom mock for [AtatClient], as mockall crate is currently not supporting the trait structure
/// due to generic const + generic closure (s. [https://github.com/asomers/mockall/issues/217]
pub struct MockAtatClient<'a> {
    /// Mocked responses which get returned in the same order as inserted
    responses: VecDeque<MockedCommand>,

    /// Publisher for URC messages
    urc_publisher: Publisher<'a, CriticalSectionRawMutex, URCMessages<16>, 16, 1, 1>,
}

/// Mocked command behaviour
pub struct MockedCommand {
    /// Expected command, None if command should not be asserted
    pub command: Option<&'static [u8]>,

    /// Sends the given response
    pub response: &'static [u8],

    /// Publishes the given URC message after the command is sent
    pub urc_messages: Option<&'static [&'static [u8]]>,
}

impl MockedCommand {
    pub fn new(
        command: Option<&'static [u8]>,
        response: &'static [u8],
        urc_messages: Option<&'static [&'static [u8]]>,
    ) -> Self {
        Self {
            command,
            response,
            urc_messages,
        }
    }

    /// Simulates an error response
    pub fn error(command: Option<&'static [u8]>, urc_messages: Option<&'static [&'static [u8]]>) -> Self {
        Self::new(command, b"ERROR\r\n", urc_messages)
    }

    /// Simulates an empty OK response
    pub fn ok(command: Option<&'static [u8]>, urc_messages: Option<&'static [&'static [u8]]>) -> Self {
        Self::new(command, b"", urc_messages)
    }
}

impl AtatClient for MockAtatClient<'_> {
    fn send<A: AtatCmd>(&mut self, cmd: &A) -> Result<A::Response, Error> {
        let mut buffer = [0x0_u8; 256];
        let length = cmd.write(&mut buffer);

        if self.responses.is_empty() {
            panic!(
                "Unexpected command {}",
                core::str::from_utf8(&buffer[..length]).unwrap()
            )
        }

        let behaviour = self.responses.pop_front().unwrap();

        if let Some(expected) = behaviour.command {
            assert_eq!(
                expected,
                &buffer[..length],
                "Expected command {} differs from actual command {}",
                core::str::from_utf8(&expected).unwrap(),
                core::str::from_utf8(&buffer[..length]).unwrap()
            );
        }

        let response = cmd.parse(Ok(behaviour.response)).map_err(|_| Error::Parse)?;

        if let Some(messages) = behaviour.urc_messages {
            for message in messages {
                if let Some(message) = URCMessages::parse(message) {
                    self.urc_publisher.try_publish(message).unwrap()
                };
            }
        }

        Ok(response)
    }
}

impl<'a> MockAtatClient<'a> {
    pub fn new(channel: &'a PubSubChannel<CriticalSectionRawMutex, URCMessages<16>, 16, 1, 1>) -> Self {
        Self {
            responses: VecDeque::new(),
            urc_publisher: channel.publisher().unwrap(),
        }
    }

    /// Adds a mock response
    pub fn add_response(&mut self, response: MockedCommand) {
        self.responses.push_back(response);
    }

    /// Publishes a URC message
    pub fn add_urc_message(&mut self, message: &'static [u8]) {
        let message = URCMessages::parse(message).unwrap();
        self.urc_publisher.try_publish(message).unwrap()
    }

    /// Simulates a 'WIFI CONNECTED' URC message
    pub fn add_urc_wifi_connected(&mut self) {
        self.add_urc_message(b"WIFI CONNECTED\r\n");
    }

    /// Simulates a 'WIFI DISCONNECT' URC message
    pub fn add_urc_wifi_disconnect(&mut self) {
        self.add_urc_message(b"WIFI DISCONNECT\r\n");
    }

    /// Simulates a 'WIFI GOT IP' URC message
    pub fn add_urc_wifi_got_ip(&mut self) {
        self.add_urc_message(b"WIFI GOT IP\r\n");
    }

    /// Simulates a 'ready' URC message
    pub fn add_urc_ready(&mut self) {
        self.add_urc_message(b"ready\r\n");
    }

    /// Simulates a connected socket state change
    pub fn add_urc_first_socket_connected(&mut self) {
        self.add_urc_message(b"0,CONNECT\r\n");
    }

    /// Simulates a connected socket state change
    pub fn add_urc_first_socket_closed(&mut self) {
        self.add_urc_message(b"0,CLOSED\r\n");
    }

    /// Asserts that there are no mocked commands left in the queue
    pub fn assert_all_cmds_sent(&self) {
        if !self.responses.is_empty() {
            panic!("Not all expected commands have been sent.");
        }
    }
}

mock! {
    pub Timer{}

    impl FugitTimer<1_000_000> for Timer {
        type Error = u32;

        fn now(&mut self) -> TimerInstantU32<1000000>;
        fn start(&mut self, duration: TimerDurationU32<1000000>) -> Result<(), u32>;
        fn cancel(&mut self) -> Result<(), u32>;
        fn wait(&mut self) -> nb::Result<(), u32>;
    }
}

impl MockTimer {
    /// Short hand helper for returning a milliseconds duration
    pub fn duration_ms(duration: u32) -> TimerDurationU32<1_000_000> {
        TimerDurationU32::millis(duration)
    }
}
