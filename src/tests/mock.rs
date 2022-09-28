use alloc::collections::VecDeque;
use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;
use atat::{AtatClient, AtatCmd, AtatUrc, Error, Mode};
use fugit::{TimerDurationU32, TimerInstantU32};
use fugit_timer::Timer as FugitTimer;
use mockall::mock;

/// Custom mock for [AtatClient], as mockall crate is currently not supportint the trait structure
/// due to generic const + generic closure (s. [https://github.com/asomers/mockall/issues/217]
pub struct MockAtatClient {
    /// Sent (encoded) commands
    commands: Vec<Vec<u8>>,

    /// Mocked responses which get returned in the same order as inserted
    responses: VecDeque<&'static [u8]>,

    /// Mocked URC messages which get returned in the same order as inserted
    urc_messages: VecDeque<&'static [u8]>,

    /// Returns no URC messages on the first N calls
    urc_skp_count: usize,

    /// If true, only one URC message is returned for one check_urc() call
    throttle_urc: bool,

    /// If true, no more URC messages get returned until next send() call
    throttle_urc_reached: bool,

    /// send() call count
    send_count: usize,

    /// Call count of
    reset_call_count: usize,

    /// Simulates a 'WouldBlock' response at given call index
    send_would_block: Option<usize>,
}

impl AtatClient for MockAtatClient {
    fn send<A: AtatCmd<LEN>, const LEN: usize>(&mut self, cmd: &A) -> nb::Result<A::Response, Error> {
        self.commands.push(cmd.as_bytes().to_vec());

        if self.send_would_block.is_some() && self.send_would_block.as_ref().unwrap() == &self.send_count {
            return nb::Result::Err(nb::Error::WouldBlock);
        }

        let response = cmd
            .parse(Ok(self.responses.pop_front().unwrap()))
            .map_err(|_| nb::Error::Other(Error::Parse))?;

        self.send_count += 1;
        self.throttle_urc_reached = false;
        nb::Result::Ok(response)
    }

    fn check_urc<URC: AtatUrc>(&mut self) -> Option<URC::Response> {
        if self.urc_skp_count > 0 {
            self.urc_skp_count -= 1;
            return None;
        }

        let mut return_urc = None;
        self.peek_urc_with::<URC, _>(|urc| {
            return_urc = Some(urc);
            true
        });
        return_urc
    }

    fn peek_urc_with<URC: AtatUrc, F: FnOnce(URC::Response) -> bool>(&mut self, f: F) {
        if self.urc_messages.is_empty() {
            return;
        }

        if self.throttle_urc && self.throttle_urc_reached {
            return;
        }

        if self.throttle_urc {
            self.throttle_urc_reached = true;
        }

        if let Some(message) = URC::parse(self.urc_messages.pop_front().unwrap()) {
            f(message);
        }
    }

    fn check_response<A: AtatCmd<LEN>, const LEN: usize>(&mut self, _cmd: &A) -> nb::Result<A::Response, Error> {
        unimplemented!("Currently not implemented for mock");
    }

    fn get_mode(&self) -> Mode {
        Mode::Timeout
    }

    fn reset(&mut self) {
        self.reset_call_count += 1;
    }
}

impl MockAtatClient {
    pub fn new() -> Self {
        Self {
            commands: vec![],
            responses: VecDeque::new(),
            urc_messages: VecDeque::new(),
            urc_skp_count: 0,
            throttle_urc: false,
            throttle_urc_reached: false,
            send_count: 0,
            reset_call_count: 0,
            send_would_block: None,
        }
    }

    /// Simulates a 'WouldBlock' response at given call index
    pub fn send_would_block(&mut self, call_index: usize) {
        self.send_count = 0;
        self.send_would_block = Some(call_index);
    }

    /// Adds a mock response
    pub fn add_response(&mut self, response: &'static [u8]) {
        self.responses.push_back(response);
    }

    /// Simulates a general error response
    pub fn add_error_response(&mut self) {
        self.add_response(b"ERROR\r\n");
    }

    /// Simulates a none response (OK)
    pub fn add_ok_response(&mut self) {
        self.add_response(b"");
    }

    /// Adds a  mock URC message
    pub fn add_urc_message(&mut self, message: &'static [u8]) {
        self.urc_messages.push_back(message);
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

    /// Simulates a unknown URC message
    pub fn add_urc_unknown(&mut self) {
        self.add_urc_message(b"UNKNOWN\r\n");
    }

    /// Simulates a 'ready' URC message
    pub fn add_urc_ready(&mut self) {
        self.add_urc_message(b"ready\r\n");
    }

    /// Simulates a connected socket state change
    pub fn add_urc_first_socket_connected(&mut self) {
        self.add_urc_message(b"0,CONNECT\r\n");
    }

    /// Simulates a 'recv 4 bytes' URC message
    pub fn add_urc_recv_bytes(&mut self) {
        self.add_urc_message(b"Recv 4 bytes\r\n");
    }

    /// Simulates a 'SEND OK' URC message
    pub fn add_urc_send_ok(&mut self) {
        self.add_urc_message(b"SEND OK\r\n");
    }

    /// Simulates a 'SEND FAIL' URC message
    pub fn add_urc_send_fail(&mut self) {
        self.add_urc_message(b"SEND FAIL\r\n");
    }

    /// Simulates a connected socket state change
    pub fn add_urc_second_socket_connected(&mut self) {
        self.add_urc_message(b"1,CONNECT\r\n");
    }

    /// Simulates a connected socket state change
    pub fn add_urc_first_socket_closed(&mut self) {
        self.add_urc_message(b"0,CLOSED\r\n");
    }

    /// Skips the given number of calls to check_urc()
    pub fn skip_urc(&mut self, count: usize) {
        self.urc_skp_count = count;
    }

    /// If set, just one URC message is processed between send() calls
    pub fn throttle_urc(&mut self) {
        self.throttle_urc = true;
    }

    /// Returns a copy of the sent commands
    pub fn get_commands_as_strings(&self) -> Vec<String> {
        let mut commands = vec![];

        for command in &self.commands {
            commands.push(String::from_utf8(command.clone()).unwrap());
        }

        commands
    }

    pub fn get_reset_call_count(&self) -> usize {
        self.reset_call_count
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
