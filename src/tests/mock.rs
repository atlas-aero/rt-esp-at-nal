use alloc::collections::VecDeque;
use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;
use atat::{AtatClient, AtatCmd, AtatUrc, Error, Mode};

/// Custom mock for [AtatClient], as mockall crate is currently not supportint the trait structure
/// due to generic const + generic closure (s. [https://github.com/asomers/mockall/issues/217]
pub struct MockAtatClient {
    /// Sent (encoded) commands
    commands: Vec<Vec<u8>>,

    /// Mocked responses which get returned in the same order as inserted
    responses: VecDeque<&'static [u8]>,

    /// Mocked URC messages which get returned in the same order as inserted
    urc_messages: VecDeque<&'static [u8]>,

    /// send() call count
    send_count: usize,

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
        nb::Result::Ok(response)
    }

    fn peek_urc_with<URC: AtatUrc, F: FnOnce(URC::Response) -> bool>(&mut self, f: F) {
        if self.urc_messages.is_empty() {
            return;
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

    fn reset(&mut self) {}
}

impl MockAtatClient {
    pub fn new() -> Self {
        Self {
            commands: vec![],
            responses: VecDeque::new(),
            urc_messages: VecDeque::new(),
            send_count: 0,
            send_would_block: None,
        }
    }

    /// Simulates a 'WouldBlock' response at given call index
    pub fn send_would_block(&mut self, call_index: usize) {
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

    /// Returns a copy of the sent commands
    pub fn get_commands_as_strings(&self) -> Vec<String> {
        let mut commands = vec![];

        for command in &self.commands {
            commands.push(String::from_utf8(command.clone()).unwrap());
        }

        commands
    }
}
