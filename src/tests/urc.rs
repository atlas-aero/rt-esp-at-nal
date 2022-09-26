use crate::urc::URCMessages;
use atat::{AtatUrc, Parser};

#[test]
fn test_first_parse_no_match() {
    let data = b"+CWJAP:\r\n";
    assert!(<URCMessages as Parser>::parse(data).is_err());
}

#[test]
fn test_first_parse_incomplete_line() {
    assert!(<URCMessages as Parser>::parse(b"").is_err());
    assert!(<URCMessages as Parser>::parse(b"\r\n").is_err());
    assert!(<URCMessages as Parser>::parse(b"OK").is_err());
    assert!(<URCMessages as Parser>::parse(b"ready\r").is_err());
    assert!(<URCMessages as Parser>::parse(b"ready\n").is_err());
}

#[test]
fn test_first_parse_too_short() {
    assert!(<URCMessages as Parser>::parse(b"OK\r\n").is_err());
}

#[test]
fn test_first_parse_ready() {
    assert_result(b"ready\r\n", 7, b"ready\r\nNEXT LINE\r\n");
    assert_result(b"ready\r\n", 7, b"ready\r\n");
    assert_result(b"ready\r\n", 7, b"ready\r\nincomplete");
}

#[test]
fn test_first_parse_empty_line() {
    assert_result(b"ready\r\n", 9, b"\r\nready\r\n");
}

#[test]
fn test_first_parse_wifi() {
    assert_result(b"WIFI CONNECTED\r\n", 16, b"WIFI CONNECTED\r\n");
    assert_result(b"WIFI CONNECTED\r\n", 16, b"WIFI CONNECTED\r\nNEXT_LINE\r\n");
    assert_result(b"WIFI CONNECTED\r\n", 16, b"WIFI CONNECTED\r\nready\r\n");
    assert_result(b"WIFI CONNECTED\r\n", 16, b"WIFI CONNECTED\r\nincomplete");

    assert_result(b"WIFI DISCONNECT\r\n", 17, b"WIFI DISCONNECT\r\n");
    assert_result(b"WIFI DISCONNECT\r\n", 17, b"WIFI DISCONNECT\r\nNEXT_LINE\r\n");
    assert_result(b"WIFI DISCONNECT\r\n", 17, b"WIFI DISCONNECT\r\nready\r\n");
    assert_result(b"WIFI DISCONNECT\r\n", 17, b"WIFI DISCONNECT\r\nincomplete");

    assert_result(b"WIFI GOT IP\r\n", 13, b"WIFI GOT IP\r\n");
    assert_result(b"WIFI GOT IP\r\n", 13, b"WIFI GOT IP\r\nNEXT_LINE\r\n");
    assert_result(b"WIFI GOT IP\r\n", 13, b"WIFI GOT IP\r\nready\r\n");
    assert_result(b"WIFI GOT IP\r\n", 13, b"WIFI GOT IP\r\nincomplete");

    assert_result(b"WIFI UNKNOWN\r\n", 14, b"WIFI UNKNOWN\r\n");
    assert_result(b"WIFI UNKNOWN\r\n", 14, b"WIFI UNKNOWN\r\nNEXT_LINE\r\n");
    assert_result(b"WIFI UNKNOWN\r\n", 14, b"WIFI UNKNOWN\r\nready\r\n");
    assert_result(b"WIFI UNKNOWN\r\n", 14, b"WIFI UNKNOWN\r\nincomplete");
}

#[test]
fn test_first_parse_connection_connected() {
    assert_result(b"0,CONNECT\r\n", 11, b"0,CONNECT\r\nNEXT LINE\r\n");
    assert_result(b"0,CONNECT\r\n", 13, b"\r\n0,CONNECT\r\n");
}

#[test]
fn test_first_parse_connection_closed() {
    assert_result(b"0,CLOSED\r\n", 10, b"0,CLOSED\r\nNEXT LINE\r\n");
    assert_result(b"0,CLOSED\r\n", 12, b"\r\n0,CLOSED\r\n");
}

#[test]
fn test_second_parse_ready() {
    assert_eq!(
        URCMessages::Ready,
        <URCMessages as AtatUrc>::parse(b"ready\r\n").unwrap()
    );
}

#[test]
fn test_second_parse_wifi_connected() {
    assert_eq!(
        URCMessages::WifiConnected,
        <URCMessages as AtatUrc>::parse(b"WIFI CONNECTED\r\n").unwrap()
    );
}

#[test]
fn test_second_parse_wifi_disconnect() {
    assert_eq!(
        URCMessages::WifiDisconnected,
        <URCMessages as AtatUrc>::parse(b"WIFI DISCONNECT\r\n").unwrap()
    );
}

#[test]
fn test_second_parse_wifi_ip_assigned() {
    assert_eq!(
        URCMessages::ReceivedIP,
        <URCMessages as AtatUrc>::parse(b"WIFI GOT IP\r\n").unwrap()
    );
}

#[test]
fn test_second_parse_wifi_unknown() {
    assert_eq!(
        URCMessages::Unknown,
        <URCMessages as AtatUrc>::parse(b"WIFI UNDEFINED\r\n").unwrap()
    );
}

#[test]
fn test_second_parse_socket_connected_valid_link_id() {
    assert_eq!(
        URCMessages::SocketConnected(0),
        <URCMessages as AtatUrc>::parse(b"0,CONNECT\r\n").unwrap()
    );
}

#[test]
fn test_second_parse_socket_connected_invalid_link_id() {
    assert!(<URCMessages as AtatUrc>::parse(b"5,CONNECT\r\n").is_none())
}

#[test]
fn test_second_parse_socket_closed_valid_link_id() {
    assert_eq!(
        URCMessages::SocketClosed(2),
        <URCMessages as AtatUrc>::parse(b"2,CLOSED\r\n").unwrap()
    );
}

#[test]
fn test_second_parse_socket_closed_invalid_link_id() {
    assert!(<URCMessages as AtatUrc>::parse(b"5,CLOSED\r\n").is_none())
}

fn assert_result(string: &[u8], size: usize, data: &[u8]) {
    match <URCMessages as Parser>::parse(data) {
        Ok(result) => {
            assert_eq!(result.0, string);
            assert_eq!(result.1, size);
        }
        Err(_) => {
            panic!("Parsed failed");
        }
    }
}
