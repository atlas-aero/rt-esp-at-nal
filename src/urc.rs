use atat::digest::ParseError;
use atat::{AtatUrc, Parser};

/// URC definitions, needs to passed as generic of [AtDigester](atat::digest::AtDigester): `AtDigester<URCMessages>`
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum URCMessages {
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
    /// Transmission of socket data was successful
    SendConfirmation,
    /// Transmission of socket data failed
    SendFail,
    /// A general error happened
    Error,
    /// Unknown URC message
    Unknown,
}

impl AtatUrc for URCMessages {
    type Response = Self;

    fn parse(resp: &[u8]) -> Option<Self::Response> {
        match &resp[1..resp.len() - 2] {
            b",CONNECT" => return Some(Self::SocketConnected(URCMessages::parse_link_id(resp[0])?)),
            b",CLOSED" => return Some(Self::SocketClosed(URCMessages::parse_link_id(resp[0])?)),
            _ => {}
        }

        if &resp[..4] == b"Recv" {
            return Some(Self::ReceivedBytes(URCMessages::parse_receive_byte_count(resp)?));
        }

        match &resp[..resp.len() - 2] {
            b"ready" => Some(Self::Ready),
            b"SEND OK" => Some(Self::SendConfirmation),
            b"SEND FAIL" => Some(Self::SendFail),
            b"ERROR" => Some(Self::Error),
            b"WIFI CONNECTED" => Some(Self::WifiConnected),
            b"WIFI DISCONNECT" => Some(Self::WifiDisconnected),
            b"WIFI GOT IP" => Some(Self::ReceivedIP),
            _ => Some(Self::Unknown),
        }
    }
}

impl URCMessages {
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
}

impl Parser for URCMessages {
    fn parse(buf: &[u8]) -> Result<(&[u8], usize), ParseError> {
        if buf.len() < 6 {
            return Err(ParseError::Incomplete);
        }

        let encoded = core::str::from_utf8(buf).map_err(|_| ParseError::NoMatch)?;
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
            if buf.len() < end {
                break;
            }

            if line == "ready"
                || line == "SEND OK"
                || line == "ERROR"
                || line == "SEND FAIL"
                || &line[..4] == "WIFI"
                || &line[1..] == ",CONNECT"
                || &line[1..] == ",CLOSED"
                || URCMessages::matches_receive_confirmation(line)
            {
                return Ok((&buf[start..end], end));
            }

            break;
        }

        Err(ParseError::NoMatch)
    }
}

impl URCMessages {
    /// Returns true if line is matching a receive confirmation e.g. "Recv 9 bytes"
    fn matches_receive_confirmation(line: &str) -> bool {
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
