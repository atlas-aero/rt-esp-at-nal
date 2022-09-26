use crate::adapter::Adapter;
use crate::stack::Error;
use crate::tests::mock::MockAtatClient;
use alloc::string::ToString;
use atat::Error as AtError;
use embedded_nal::TcpClientStack;

#[test]
fn test_socket_multi_conn_error() {
    let mut client = MockAtatClient::new();
    client.add_error_response();

    let mut adapter = Adapter::new(client);
    let result = adapter.socket().unwrap_err();
    assert_eq!(Error::EnablingMultiConnectionsFailed(AtError::Parse), result);

    let commands = adapter.client.get_commands_as_strings();
    assert_eq!(1, commands.len());
    assert_eq!("AT+CIPMUX=1\r\n".to_string(), commands[0]);
}

#[test]
fn test_socket_multi_conn_would_block() {
    let mut client = MockAtatClient::new();
    client.send_would_block(0);

    let mut adapter = Adapter::new(client);
    let result = adapter.socket().unwrap_err();

    assert_eq!(Error::UnexpectedWouldBlock, result);
}

#[test]
fn test_socket_multi_conn_enabled_once() {
    let mut client = MockAtatClient::new();
    client.add_ok_response();

    let mut adapter = Adapter::new(client);
    adapter.socket().unwrap();
    adapter.socket().unwrap();

    let commands = adapter.client.get_commands_as_strings();
    assert_eq!(1, commands.len());
    assert_eq!("AT+CIPMUX=1\r\n".to_string(), commands[0]);
}

#[test]
fn test_socket_opened() {
    let mut client = MockAtatClient::new();
    client.add_ok_response();

    let mut adapter = Adapter::new(client);
    assert_eq!(0, adapter.socket().unwrap().link_id);
    assert_eq!(1, adapter.socket().unwrap().link_id);
    assert_eq!(2, adapter.socket().unwrap().link_id);
    assert_eq!(3, adapter.socket().unwrap().link_id);
    assert_eq!(4, adapter.socket().unwrap().link_id);
}

#[test]
fn test_socket_not_available() {
    let mut client = MockAtatClient::new();
    client.add_ok_response();

    let mut adapter = Adapter::new(client);
    for _ in 0..5 {
        adapter.socket().unwrap();
    }

    let result = adapter.socket().unwrap_err();
    assert_eq!(Error::NoSocketAvailable, result);
}
