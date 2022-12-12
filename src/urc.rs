//! # URC parser implementation
//!
//! This is just used internally, but needs to be public for passing [URCMessages] as a generic to
//! [AtDigester](atat::digest::AtDigester): `AtDigester<URCMessages>`.
use atat::digest::ParseError;
use atat::{AtatUrc, Parser};
use heapless::Vec;

/// URC definitions, needs to passed as generic of [AtDigester](atat::digest::AtDigester): `AtDigester<URCMessages>`
#[derive(Debug, PartialEq, Eq)]
pub enum URCMessages<const RX_SIZE: usize> {
    /// Modem is ready for receiving AT commands
    Ready,
    /// WIFi connection state changed to to connected
    WifiConnected,
    /// Wifi connection state changed to disconnected
    WifiDisconnected,
    /// Received an IP from the access point
    ReceivedIP,
    /// Socket with the given link_id connected
    SocketConnected(usize),
    /// Socket with the given link_id closed
    SocketClosed(usize),
    /// Confirmation that the given number of bytes have been received by ESP-AT
    ReceivedBytes(usize),
    /// Signals that socket is already connected when trying to establish the same connection again
    AlreadyConnected,
    /// Transmission of socket data was successful
    SendConfirmation,
    /// Transmission of socket data failed
    SendFail,
    /// Data is available in passive receiving mode.
    /// First value = link_id, Second value = available byte count
    DataAvailable(usize, usize),
    /// Received the following data requested by CIPRECVDATA command.
    Data(Vec<u8, RX_SIZE>),
    /// Echo of a command
    Echo,
    /// Unknown URC message
    Unknown,
}

impl<const RX_SIZE: usize> AtatUrc for URCMessages<RX_SIZE> {
    type Response = Self;

    fn parse(resp: &[u8]) -> Option<Self::Response> {
        // Command echo
        if &resp[..3] == b"AT+" {
            return Some(Self::Echo);
        }

        if &resp[..4] == b"+IPD" {
            return URCMessages::parse_data_available(resp);
        }

        if resp.len() > 15 && &resp[..13] == b"+CIPRECVDATA," {
            let message = DataResponseParser::new(resp).parse().ok()?;
            return Some(Self::Data(message.to_vec()?));
        }

        match &resp[1..resp.len() - 2] {
            b",CONNECT" => return Some(Self::SocketConnected(URCMessages::<8>::parse_link_id(resp[0])?)),
            b",CLOSED" => return Some(Self::SocketClosed(URCMessages::<8>::parse_link_id(resp[0])?)),
            _ => {}
        }

        if &resp[..4] == b"Recv" {
            return Some(Self::ReceivedBytes(URCMessages::<8>::parse_receive_byte_count(resp)?));
        }

        match &resp[..resp.len() - 2] {
            b"ready" => Some(Self::Ready),
            b"SEND OK" => Some(Self::SendConfirmation),
            b"SEND FAIL" => Some(Self::SendFail),
            b"WIFI CONNECTED" => Some(Self::WifiConnected),
            b"WIFI DISCONNECT" => Some(Self::WifiDisconnected),
            b"WIFI GOT IP" => Some(Self::ReceivedIP),
            b"ALREADY CONNECTED" => Some(Self::AlreadyConnected),
            _ => Some(Self::Unknown),
        }
    }
}

impl<const RX_SIZE: usize> URCMessages<RX_SIZE> {
    /// Parses the socket id. Currently supports just socket 0-4
    fn parse_link_id(link_id: u8) -> Option<usize> {
        match link_id {
            0x30 => Some(0),
            0x31 => Some(1),
            0x32 => Some(2),
            0x33 => Some(3),
            0x34 => Some(4),
            _ => None,
        }
    }

    /// Tries to parse the N byte count of 'Recv N bytes'
    fn parse_receive_byte_count(resp: &[u8]) -> Option<usize> {
        let postfix_start = resp.len() - 8;
        let byte_count = &resp[5..postfix_start];

        if let Ok(string) = core::str::from_utf8(byte_count) {
            if let Ok(byte_count) = string.parse::<usize>() {
                return Some(byte_count);
            }
        }

        None
    }

    /// Parses the +IPD message
    fn parse_data_available(data: &[u8]) -> Option<Self> {
        let string = core::str::from_utf8(&data[..data.len() - 2]).ok()?;
        let mut parts = string.split(',');

        let link_id = parts.nth(1)?.parse().ok()?;
        let length = parts.last()?.parse().ok()?;

        Some(Self::DataAvailable(link_id, length))
    }
}

impl<const RX_SIZE: usize> Parser for URCMessages<RX_SIZE> {
    fn parse(buf: &[u8]) -> Result<(&[u8], usize), ParseError> {
        if buf.len() < 6 {
            return Err(ParseError::Incomplete);
        }

        if let Some(matcher) = SizeBasedMatcher::matches(buf) {
            return matcher.handle();
        }

        if let Ok(result) = LineBasedMatcher::new(buf).handle() {
            return Ok(result);
        }

        BootMessageParser::new(buf).handle()
    }
}

/// Matches length defined URC message +CIPRECVDATA
struct SizeBasedMatcher<'a> {
    buffer: &'a [u8],

    /// First index where the actual message starts
    start: usize,
}

impl<'a> SizeBasedMatcher<'a> {
    /// Returns Self if buffer contains a sized encoded message
    pub fn matches(buffer: &'a [u8]) -> Option<Self> {
        if buffer.len() < 15 {
            return None;
        }

        let start = buffer.iter().enumerate().find(|x| x.1 != &b'\r' && x.1 != &b'\n')?.0;

        let data = &buffer[start..];
        if data.len() < 13 || &data[..13] != b"+CIPRECVDATA," {
            return None;
        }

        Some(Self { buffer, start })
    }

    /// Parses the message and checks if data is complete
    pub fn handle(self) -> Result<(&'a [u8], usize), ParseError> {
        let data = &self.buffer[self.start..];
        let message = DataResponseParser::new(data).parse()?;

        let total_length = self.start + 13 + message.length_str.len() + 1 + message.length;
        Ok((&data[..total_length - self.start], total_length))
    }
}

/// Matches regular CRLF terminated URC messages
struct LineBasedMatcher<'a> {
    buffer: &'a [u8],
}

impl<'a> LineBasedMatcher<'a> {
    pub fn new(buffer: &'a [u8]) -> Self {
        Self { buffer }
    }

    /// Handles regular CRLF terminated URC message
    fn handle(self) -> Result<(&'a [u8], usize), ParseError> {
        let encoded = core::str::from_utf8(self.buffer).map_err(|_| ParseError::NoMatch)?;
        let mut start = 0;
        let mut end = 0;

        for line in encoded.split("\r\n") {
            if line.is_empty() {
                start += 2;
                end += 2;
                continue;
            }

            // Min. line length for matching any needles
            if line.len() < 4 {
                break;
            }

            end += line.len() + 2;

            // Line does not end with CRLF
            if self.buffer.len() < end {
                break;
            }

            if self.matches_lines_based_urc(line) {
                return Ok((&self.buffer[start..end], end));
            }
            break;
        }

        Err(ParseError::NoMatch)
    }

    /// True if a regular CRLF terminated URC message was matched
    fn matches_lines_based_urc(&self, line: &str) -> bool {
        line == "ready"
            || &line[..3] == "AT+"
            || &line[..4] == "+IPD"
            || line == "SEND OK"
            || line == "SEND FAIL"
            || &line[..4] == "WIFI"
            || &line[1..] == ",CONNECT"
            || &line[1..] == ",CLOSED"
            || line == "ALREADY CONNECTED"
            || self.matches_receive_confirmation(line)
    }

    /// Returns true if line is matching a receive confirmation e.g. "Recv 9 bytes"
    fn matches_receive_confirmation(&self, line: &str) -> bool {
        if line.len() < 12 {
            return false;
        }

        if &line[..4] != "Recv" {
            return false;
        }

        let postfix_start = line.len() - 6;
        &line[postfix_start..] != "bytes"
    }
}

/// Decodes a +CIPRECVDATA message
struct DataResponseParser<'a> {
    buffer: &'a [u8],
}

impl<'a> DataResponseParser<'a> {
    pub fn new(buffer: &'a [u8]) -> Self {
        Self { buffer }
    }

    /// Parses the length and returns both the usize length + length string
    pub fn parse(self) -> Result<DataMessage<'a>, ParseError> {
        let separator = self
            .buffer
            .iter()
            .enumerate()
            .find(|x| x.1 == &b':')
            .ok_or(ParseError::Incomplete)?
            .0;
        let length_str = core::str::from_utf8(&self.buffer[13..separator]).map_err(|_| ParseError::NoMatch)?;
        let length_usize = length_str.parse::<usize>().map_err(|_| ParseError::NoMatch)?;

        let remaining_data = &self.buffer[separator + 1..];
        if remaining_data.len() < length_usize {
            return Err(ParseError::Incomplete);
        }

        Ok(DataMessage {
            length: length_usize,
            length_str,
            data: remaining_data,
        })
    }
}

/// Decoded data message
struct DataMessage<'a> {
    /// Serial data length
    pub length: usize,

    /// Serial data length as string
    pub length_str: &'a str,

    /// All data after separator
    pub data: &'a [u8],
}

impl<'a> DataMessage<'a> {
    /// Copies all serial data to a vector
    fn to_vec<const LEN: usize>(&self) -> Option<Vec<u8, LEN>> {
        let mut vec = Vec::new();
        vec.extend_from_slice(self.data).ok()?;

        Some(vec)
    }
}

/// Parser for boot messages
struct BootMessageParser<'a> {
    buffer: &'a [u8],
}

impl<'a> BootMessageParser<'a> {
    pub fn new(buffer: &'a [u8]) -> Self {
        Self { buffer }
    }

    /// Matches if a boot sequence is detected an a ready message is found
    pub fn handle(self) -> Result<(&'a [u8], usize), ParseError> {
        let mut is_boot_seq = false;
        let mut size = 0;

        for line in self.buffer.split(|b| b == &b'\n') {
            size += line.len() + 1;

            if !is_boot_seq && self.is_boot_line(line) {
                is_boot_seq = true;
                continue;
            }

            if is_boot_seq && line == b"ready\r" {
                return Ok((b"ready\r\n", size));
            }
        }

        Err(ParseError::NoMatch)
    }

    /// Returns true if a boot line like "ets Jan  8 2013,rst cause:1, boot mode:(3,7)" is found
    fn is_boot_line(&self, line: &[u8]) -> bool {
        if let Ok(decoded) = core::str::from_utf8(line) {
            return decoded.contains("rst cause:");
        }

        false
    }
}
