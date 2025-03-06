use crate::urc::URCMessages;
use atat::{AtatUrc, Parser};
use heapless::Vec;

#[test]
fn test_first_parse_no_match() {
    let data = b"+CWJAP:\r\n";
    assert!(<URCMessages<32> as Parser>::parse(data).is_err());
}

#[test]
fn test_first_parse_incomplete_line() {
    assert!(<URCMessages<32> as Parser>::parse(b"").is_err());
    assert!(<URCMessages<32> as Parser>::parse(b"\r\n").is_err());
    assert!(<URCMessages<32> as Parser>::parse(b"OK").is_err());
    assert!(<URCMessages<32> as Parser>::parse(b"ready\r").is_err());
    assert!(<URCMessages<32> as Parser>::parse(b"ready\n").is_err());
    assert!(<URCMessages<32> as Parser>::parse(b"ready").is_err());
    assert!(<URCMessages<32> as Parser>::parse(b"+IPD").is_err());
    assert!(<URCMessages<32> as Parser>::parse(b"\r\n+IPD").is_err());
    assert!(<URCMessages<32> as Parser>::parse(b"+IPD,5").is_err());
    assert!(<URCMessages<32> as Parser>::parse(b"+IPD,5,100").is_err());
}

#[test]
fn test_first_parse_previous_ok() {
    assert!(<URCMessages<32> as Parser>::parse(b"OK\r\n").is_err());
    assert!(<URCMessages<32> as Parser>::parse(b"OK\r\nWIFI GOT IP\r\n").is_err());
    assert!(<URCMessages<32> as Parser>::parse(b"OK\r\n+CIPRECVDATA,5:abcde").is_err());
    assert!(<URCMessages<32> as Parser>::parse(b"OK\r\n+CIPRECVDATA:5,abcde").is_err());
}

#[test]
fn test_first_parse_too_short() {
    assert!(<URCMessages<32> as Parser>::parse(b"OK\r\n").is_err());
    assert!(<URCMessages<32> as Parser>::parse(b"\r\n\r\n\r\nOK\r\n").is_err());
    assert!(<URCMessages<32> as Parser>::parse(b"\r\n\r\n\r\nOK").is_err());
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

/// As WIFI status messages are sometimes not properly terminated by double CLRF
/// CLRF ist not removed from the buffer, so length-2 is returned
#[test]
fn test_first_parse_wifi() {
    assert_result(b"WIFI CONNECTED\r\n", 14, b"WIFI CONNECTED\r\n");
    assert_result(b"WIFI CONNECTED\r\n", 14, b"WIFI CONNECTED\r\nNEXT_LINE\r\n");
    assert_result(b"WIFI CONNECTED\r\n", 14, b"WIFI CONNECTED\r\nready\r\n");
    assert_result(b"WIFI CONNECTED\r\n", 14, b"WIFI CONNECTED\r\nincomplete");

    assert_result(b"WIFI DISCONNECT\r\n", 15, b"WIFI DISCONNECT\r\n");
    assert_result(b"WIFI DISCONNECT\r\n", 15, b"WIFI DISCONNECT\r\nNEXT_LINE\r\n");
    assert_result(b"WIFI DISCONNECT\r\n", 15, b"WIFI DISCONNECT\r\nready\r\n");
    assert_result(b"WIFI DISCONNECT\r\n", 15, b"WIFI DISCONNECT\r\nincomplete");

    assert_result(b"WIFI GOT IP\r\n", 11, b"WIFI GOT IP\r\n");
    assert_result(b"WIFI GOT IP\r\n", 11, b"WIFI GOT IP\r\nNEXT_LINE\r\n");
    assert_result(b"WIFI GOT IP\r\n", 11, b"WIFI GOT IP\r\nready\r\n");
    assert_result(b"WIFI GOT IP\r\n", 11, b"WIFI GOT IP\r\nincomplete");

    assert_result(b"WIFI UNKNOWN\r\n", 12, b"WIFI UNKNOWN\r\n");
    assert_result(b"WIFI UNKNOWN\r\n", 12, b"WIFI UNKNOWN\r\nNEXT_LINE\r\n");
    assert_result(b"WIFI UNKNOWN\r\n", 12, b"WIFI UNKNOWN\r\nready\r\n");
    assert_result(b"WIFI UNKNOWN\r\n", 12, b"WIFI UNKNOWN\r\nincomplete");
}

#[test]
fn test_first_parse_connection_connected() {
    assert_result(b"0,CONNECT\r\n", 11, b"0,CONNECT\r\nNEXT LINE\r\n");
    assert_result(b"0,CONNECT\r\n", 15, b"\r\n\r\n0,CONNECT\r\n");
}

#[test]
fn test_first_parse_connection_closed() {
    assert_result(b"0,CLOSED\r\n", 10, b"0,CLOSED\r\nNEXT LINE\r\n");
    assert_result(b"0,CLOSED\r\n", 12, b"\r\n0,CLOSED\r\n");
    assert_result(b"0,CLOSED\r\n", 14, b"\r\n\r\n0,CLOSED\r\n");
}

#[test]
fn test_first_parse_already_connected() {
    assert_result(b"ALREADY CONNECTED\r\n", 19, b"ALREADY CONNECTED\r\n\r\n");
    assert_result(b"ALREADY CONNECTED\r\n", 21, b"\r\nALREADY CONNECTED\r\n\r\n");
}

#[test]
fn test_first_parse_receive_confirmation() {
    assert_result(b"Recv 9 bytes\r\n", 14, b"Recv 9 bytes\r\n");
    assert_result(b"Recv 9 bytes\r\n", 16, b"\r\nRecv 9 bytes\r\n");
    assert_result(b"Recv 34 bytes\r\n", 15, b"Recv 34 bytes\r\n");
    assert_result(b"Recv 999 bytes\r\n", 16, b"Recv 999 bytes\r\n");
}

#[test]
fn test_first_parse_send_ok() {
    assert_result(b"SEND OK\r\n", 9, b"SEND OK\r\n");
    assert_result(b"SEND OK\r\n", 15, b"\r\n\r\n\r\nSEND OK\r\n");
    assert_result(b"SEND OK\r\n", 9, b"SEND OK\r\n+CIPRECVDATA,5:abcde");
}

#[test]
fn test_first_parse_send_fail() {
    assert_result(b"SEND FAIL\r\n", 11, b"SEND FAIL\r\n");
    assert_result(b"SEND FAIL\r\n", 17, b"\r\n\r\n\r\nSEND FAIL\r\n");
}

#[test]
fn test_first_parse_data_available() {
    assert_result(b"+IPD,0,100\r\n", 12, b"+IPD,0,100\r\n");
    assert_result(b"+IPD,4,2048\r\n", 13, b"+IPD,4,2048\r\n");
    assert_result(b"+IPD,0,100\r\n", 18, b"\r\n\r\n\r\n+IPD,0,100\r\n");
}

#[test]
fn test_first_parse_data_prefix_incomplete() {
    assert!(<URCMessages<32> as Parser>::parse(b"+CIPRECVDATA").is_err());
    assert!(<URCMessages<32> as Parser>::parse(b"\r\n+CIPRECVDATA,").is_err());
    assert!(<URCMessages<32> as Parser>::parse(b"+CIPRECVDATA,").is_err());
    assert!(<URCMessages<32> as Parser>::parse(b"+CIPRECVDATA,6").is_err());
    assert!(<URCMessages<32> as Parser>::parse(b"\r\n+CIPRECVDATA:").is_err());
    assert!(<URCMessages<32> as Parser>::parse(b"+CIPRECVDATA:").is_err());
    assert!(<URCMessages<32> as Parser>::parse(b"+CIPRECVDATA:6").is_err());
}

#[test]
fn test_first_parse_data_serial_data_incomplete() {
    assert!(<URCMessages<32> as Parser>::parse(b"+CIPRECVDATA,5:abcd").is_err());
    assert!(<URCMessages<32> as Parser>::parse(b"\r\n+CIPRECVDATA,5:abcd").is_err());
    assert!(<URCMessages<32> as Parser>::parse(b"+CIPRECVDATA:5,abcd").is_err());
    assert!(<URCMessages<32> as Parser>::parse(b"\r\n+CIPRECVDATA:5,abcd").is_err());
}

#[test]
fn test_first_parse_boot_incomplete() {
    assert!(<URCMessages<32> as Parser>::parse(b"ets Jan  8 2013,rst cause:1, boot mode:(3,7)").is_err());
    assert!(<URCMessages<32> as Parser>::parse(b"ets Jan  8 2013,rst cause:1, boot mode:(3,7)\r\n\n\r\n").is_err());
    assert!(<URCMessages<32> as Parser>::parse(b"tail 0\r\nready\r\n").is_err());
}

#[test]
fn test_first_parse_boot_matches() {
    assert_result(b"ready\r\n", 95, b"\r\nets Jan  8 2013,rst cause:1, boot mode:(3,7)\r\n\r\nload 0x40100000, len 2592, room 16\r\n\r\nready\r\nWIFI GOT IP\r\n");
}

#[test]
fn test_first_parse_data_fully_received() {
    // Out of spec implementation for covering older ESP-AT version bug
    assert_result(b"+CIPRECVDATA,5:abcde", 20, b"+CIPRECVDATA,5:abcde\r\n\r\nOK\r\n");
    assert_result(
        b"+CIPRECVDATA,10:abcdefghij",
        26,
        b"+CIPRECVDATA,10:abcdefghij\r\n\r\nOK\r\n",
    );
    assert_result(b"+CIPRECVDATA,8:abcde\r\nH", 23, b"+CIPRECVDATA,8:abcde\r\nH\r\nOK\r\n");
    assert_result(b"+CIPRECVDATA,5:abcde", 20, b"+CIPRECVDATA,5:abcde");
    assert_result(b"+CIPRECVDATA,5:abcde", 24, b"\r\n\r\n+CIPRECVDATA,5:abcde");

    // Actual correct response according to spec
    assert_result(b"+CIPRECVDATA:5,abcde", 20, b"+CIPRECVDATA:5,abcde\r\n\r\nOK\r\n");
    assert_result(
        b"+CIPRECVDATA:10,abcdefghij",
        26,
        b"+CIPRECVDATA:10,abcdefghij\r\n\r\nOK\r\n",
    );
    assert_result(b"+CIPRECVDATA:8,abcde\r\nH", 23, b"+CIPRECVDATA:8,abcde\r\nH\r\nOK\r\n");
    assert_result(b"+CIPRECVDATA:5,abcde", 20, b"+CIPRECVDATA:5,abcde");
    assert_result(b"+CIPRECVDATA:5,abcde", 24, b"\r\n\r\n+CIPRECVDATA:5,abcde");
}

#[test]
fn test_second_parse_ready() {
    assert_eq!(
        URCMessages::Ready,
        <URCMessages<32> as AtatUrc>::parse(b"ready\r\n").unwrap()
    );
}

#[test]
fn test_second_parse_wifi_connected() {
    assert_eq!(
        URCMessages::WifiConnected,
        <URCMessages<32> as AtatUrc>::parse(b"WIFI CONNECTED\r\n").unwrap()
    );
}

#[test]
fn test_second_parse_wifi_disconnect() {
    assert_eq!(
        URCMessages::WifiDisconnected,
        <URCMessages<32> as AtatUrc>::parse(b"WIFI DISCONNECT\r\n").unwrap()
    );
}

#[test]
fn test_second_parse_wifi_ip_assigned() {
    assert_eq!(
        URCMessages::ReceivedIP,
        <URCMessages<32> as AtatUrc>::parse(b"WIFI GOT IP\r\n").unwrap()
    );
}

#[test]
fn test_second_parse_wifi_unknown() {
    assert_eq!(
        URCMessages::Unknown,
        <URCMessages<32> as AtatUrc>::parse(b"WIFI UNDEFINED\r\n").unwrap()
    );
}

#[test]
fn test_second_parse_socket_connected_valid_link_id() {
    assert_eq!(
        URCMessages::SocketConnected(0),
        <URCMessages<32> as AtatUrc>::parse(b"0,CONNECT\r\n").unwrap()
    );
}

#[test]
fn test_second_parse_socket_connected_invalid_link_id() {
    assert!(<URCMessages<32> as AtatUrc>::parse(b"5,CONNECT\r\n").is_none())
}

#[test]
fn test_second_parse_socket_closed_valid_link_id() {
    assert_eq!(
        URCMessages::SocketClosed(2),
        <URCMessages<32> as AtatUrc>::parse(b"2,CLOSED\r\n").unwrap()
    );
}

#[test]
fn test_second_parse_socket_closed_invalid_link_id() {
    assert!(<URCMessages<32> as AtatUrc>::parse(b"5,CLOSED\r\n").is_none())
}

#[test]
fn test_second_parse_already_connected() {
    assert_eq!(
        URCMessages::AlreadyConnected,
        <URCMessages<32> as AtatUrc>::parse(b"ALREADY CONNECTED\r\n").unwrap()
    );
}

#[test]
fn test_second_parse_received_bytes_valid_byte_count() {
    assert_eq!(
        URCMessages::ReceivedBytes(124),
        <URCMessages<32> as AtatUrc>::parse(b"Recv 124 bytes\r\n").unwrap()
    );
}

#[test]
fn test_second_parse_received_bytes_valid_invalid_byte_count() {
    assert!(<URCMessages<32> as AtatUrc>::parse(b"Recv -55 bytes\r\n").is_none());
    assert!(<URCMessages<32> as AtatUrc>::parse(b"Recv A bytes\r\n").is_none());
}

#[test]
fn test_second_parse_send_ok() {
    assert_eq!(
        URCMessages::SendConfirmation,
        <URCMessages<32> as AtatUrc>::parse(b"SEND OK\r\n").unwrap()
    );
}

#[test]
fn test_second_parse_send_fail() {
    assert_eq!(
        URCMessages::SendFail,
        <URCMessages<32> as AtatUrc>::parse(b"SEND FAIL\r\n").unwrap()
    );
}

#[test]
fn test_second_parse_data_available_correct() {
    assert_eq!(
        URCMessages::DataAvailable(3, 256),
        <URCMessages<32> as AtatUrc>::parse(b"+IPD,3,256\r\n").unwrap()
    );
}

#[test]
fn test_second_parse_data_available_incomplete() {
    assert!(<URCMessages<32> as AtatUrc>::parse(b"+IPD,3,\r\n").is_none());
    assert!(<URCMessages<32> as AtatUrc>::parse(b"+IPD,,200\r\n").is_none());
    assert!(<URCMessages<32> as AtatUrc>::parse(b"+IPD,3\r\n").is_none());
    assert!(<URCMessages<32> as AtatUrc>::parse(b"+IPD,\r\n").is_none());
    assert!(<URCMessages<32> as AtatUrc>::parse(b"+IPD\r\n").is_none());
}

#[test]
fn test_second_parse_data_available_invalid_numbers() {
    assert!(<URCMessages<32> as AtatUrc>::parse(b"+IPD,3,A\r\n").is_none());
    assert!(<URCMessages<32> as AtatUrc>::parse(b"+IPD,A,200\r\n").is_none());
    assert!(<URCMessages<32> as AtatUrc>::parse(b"+IPD,-1,200\r\n").is_none());
    assert!(<URCMessages<32> as AtatUrc>::parse(b"+IPD,0,-5\r\n").is_none());
}

#[test]
fn test_second_parse_data() {
    // Out of spec implementation for covering older ESP-AT version bug
    assert_eq!(
        URCMessages::<32>::Data(Vec::from_slice(b"abcde").unwrap()),
        <URCMessages<32> as AtatUrc>::parse(b"+CIPRECVDATA,5:abcde").unwrap()
    );

    // Actual correct response according to spec
    assert_eq!(
        URCMessages::<32>::Data(Vec::from_slice(b"abcde").unwrap()),
        <URCMessages<32> as AtatUrc>::parse(b"+CIPRECVDATA:5,abcde").unwrap()
    );
}

#[test]
fn test_second_parse_longer_then_block_size() {
    // Basically this can just happen if ESP-AT sends more data then requested, which is a protocol violation
    assert!(<URCMessages<4> as AtatUrc>::parse(b"+CIPRECVDATA,5:abcde").is_none());
    assert!(<URCMessages<4> as AtatUrc>::parse(b"+CIPRECVDATA:5,abcde").is_none());
}

fn assert_result(string: &[u8], size: usize, data: &[u8]) {
    match <URCMessages<32> as Parser>::parse(data) {
        Ok(result) => {
            assert_eq!(result.0, string);
            assert_eq!(result.1, size);
        }
        Err(_) => {
            panic!(
                "Parsing '{:?}' failed",
                core::str::from_utf8(data).unwrap_or("<invalid UTF-8>")
            );
        }
    }
}
