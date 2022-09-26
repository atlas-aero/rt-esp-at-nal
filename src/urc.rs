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
    /// Socket with the given link_id connected
    SocketConnected(usize),
    /// Socket with the given link_id closed
    SocketClosed(usize),
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

        match &resp[..resp.len() - 2] {
            b"ready" => Some(Self::Ready),
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
}

const KEY_READY: &str = "ready";
const KEY_WIFI: &str = "WIFI";

const CONN_CONNECT_PREFIX: &str = ",CONNECT";
const CONN_CLOSE_PREFIX: &str = ",CLOSED";

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

        let ok_return = Ok((&buf[start..eof + 2], eof + 2));

        if line == KEY_READY || &line[..4] == KEY_WIFI {
            return ok_return;
        }

        if &line[1..] == CONN_CONNECT_PREFIX || &line[1..] == CONN_CLOSE_PREFIX {
            return ok_return;
        }

        Err(ParseError::NoMatch)
    }
}
