use atat::digest::ParseError;
use atat::{AtatUrc, Parser};
use core::ops::Add;

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
    /// Unknown URC message
    Unknown,
}

impl AtatUrc for URCMessages {
    type Response = Self;

    fn parse(resp: &[u8]) -> Option<Self::Response> {
        match &resp[..resp.len() - 2] {
            b"ready" => Some(Self::Ready),
            b"WIFI CONNECTED" => Some(Self::WifiConnected),
            b"WIFI DISCONNECT" => Some(Self::WifiDisconnected),
            b"WIFI GOT IP" => Some(Self::ReceivedIP),
            _ => Some(Self::Unknown),
        }
    }
}

const KEY_READY: &str = "ready";
const KEY_WIFI: &str = "WIFI";

impl Parser for URCMessages {
    fn parse(buf: &[u8]) -> Result<(&[u8], usize), ParseError> {
        if buf.len() < 2 {
            return Err(ParseError::Incomplete);
        }

        let encoded = core::str::from_utf8(buf).map_err(|_| ParseError::NoMatch)?;
        let eof = encoded[2..].find("\r\n").ok_or(ParseError::NoMatch)?.add(2);
        let mut line = &encoded[..eof];

        // Line start
        let mut start = 0;

        // Remove empty line
        if &line[..2] == "\r\n" {
            start = 2;
            line = &line[2..];
        }

        if line.len() < 5 {
            return Err(ParseError::NoMatch);
        }

        if line == KEY_READY || &line[..4] == KEY_WIFI {
            return Ok((&buf[start..eof + 2], eof + 2));
        }

        Err(ParseError::NoMatch)
    }
}
