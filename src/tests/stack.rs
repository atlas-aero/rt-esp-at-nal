use crate::adapter::Adapter;
use crate::stack::Error;
use crate::tests::mock::MockAtatClient;
use alloc::string::ToString;
use atat::Error as AtError;
use core::str::FromStr;
use embedded_nal::{SocketAddr, TcpClientStack};

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

#[test]
fn test_connect_already_connected_by_urc() {
    let mut client = MockAtatClient::new();

    // Multiple connections command
    client.add_ok_response();

    client.add_urc_first_socket_connected();
    let mut adapter = Adapter::new(client);

    let mut socket = adapter.socket().unwrap();
    let error = adapter
        .connect(&mut socket, SocketAddr::from_str("127.0.0.1:5000").unwrap())
        .unwrap_err();

    assert_eq!(nb::Error::Other(Error::AlreadyConnected), error);
}

#[test]
fn test_connect_correct_commands_ipv4() {
    let mut client = MockAtatClient::new();

    // Multiple connections command
    client.add_ok_response();
    // Receiving mode command
    client.add_ok_response();
    // Connect command
    client.add_ok_response();

    client.skip_urc(1);
    client.add_urc_first_socket_connected();

    let mut adapter = Adapter::new(client);

    let mut socket = adapter.socket().unwrap();
    adapter
        .connect(&mut socket, SocketAddr::from_str("127.0.0.1:5000").unwrap())
        .unwrap();

    let commands = adapter.client.get_commands_as_strings();
    assert_eq!(3, commands.len());
    assert_eq!("AT+CIPRECVMODE=1\r\n".to_string(), commands[1]);
    assert_eq!("AT+CIPSTART=0,\"TCP\",\"127.0.0.1\",5000\r\n".to_string(), commands[2]);
}

#[test]
fn test_connect_correct_commands_ipv6() {
    let mut client = MockAtatClient::new();

    // Multiple connections command
    client.add_ok_response();
    // Receiving mode command
    client.add_ok_response();
    // Connect command
    client.add_ok_response();

    client.skip_urc(1);
    client.add_urc_first_socket_connected();

    let mut adapter = Adapter::new(client);

    let mut socket = adapter.socket().unwrap();
    adapter
        .connect(&mut socket, SocketAddr::from_str("[2001:db8::1]:8080").unwrap())
        .unwrap();

    let commands = adapter.client.get_commands_as_strings();
    assert_eq!(3, commands.len());
    assert_eq!("AT+CIPRECVMODE=1\r\n".to_string(), commands[1]);
    assert_eq!(
        "AT+CIPSTART=0,\"TCPv6\",\"2001:db8::1\",8080\r\n".to_string(),
        commands[2]
    );
}

#[test]
fn test_connect_receive_mode_error() {
    let mut client = MockAtatClient::new();

    // Multiple connections command
    client.add_ok_response();
    // Receiving mode command
    client.add_error_response();

    let mut adapter = Adapter::new(client);

    let mut socket = adapter.socket().unwrap();
    let error = adapter
        .connect(&mut socket, SocketAddr::from_str("127.0.0.1:5000").unwrap())
        .unwrap_err();

    assert_eq!(
        nb::Error::Other(Error::EnablingPassiveSocketModeFailed(AtError::Parse)),
        error
    );
}

#[test]
fn test_connect_receive_mode_would_block() {
    let mut client = MockAtatClient::new();

    // Multiple connections command
    client.add_ok_response();
    // Receiving mode command
    client.add_ok_response();
    // Connect mode command
    client.send_would_block(2);

    let mut adapter = Adapter::new(client);

    let mut socket = adapter.socket().unwrap();
    let error = adapter
        .connect(&mut socket, SocketAddr::from_str("127.0.0.1:5000").unwrap())
        .unwrap_err();

    assert_eq!(nb::Error::Other(Error::UnexpectedWouldBlock), error);
}

#[test]
fn test_connect_connect_command_error() {
    let mut client = MockAtatClient::new();

    // Multiple connections command
    client.add_ok_response();
    // Receiving mode command
    client.add_ok_response();
    // Connect command
    client.add_error_response();

    let mut adapter = Adapter::new(client);

    let mut socket = adapter.socket().unwrap();
    let error = adapter
        .connect(&mut socket, SocketAddr::from_str("127.0.0.1:5000").unwrap())
        .unwrap_err();

    assert_eq!(nb::Error::Other(Error::ConnectError(AtError::Parse)), error);
}

#[test]
fn test_connect_connect_command_would_block() {
    let mut client = MockAtatClient::new();

    // Multiple connections command
    client.add_ok_response();
    // Receiving mode command
    client.send_would_block(1);

    let mut adapter = Adapter::new(client);

    let mut socket = adapter.socket().unwrap();
    let error = adapter
        .connect(&mut socket, SocketAddr::from_str("127.0.0.1:5000").unwrap())
        .unwrap_err();

    assert_eq!(nb::Error::Other(Error::UnexpectedWouldBlock), error);
}

#[test]
fn test_connect_receiving_mode_cmd_sent_once() {
    let mut client = MockAtatClient::new();

    // Multiple connections command
    client.add_ok_response();

    // Receiving mode command
    client.add_ok_response();

    // First connect command
    client.add_ok_response();
    client.skip_urc(1);
    client.add_urc_first_socket_connected();

    let mut adapter = Adapter::new(client);

    let mut socket1 = adapter.socket().unwrap();
    let mut socket2 = adapter.socket().unwrap();

    adapter
        .connect(&mut socket1, SocketAddr::from_str("127.0.0.1:5000").unwrap())
        .unwrap();

    // Second connect command
    adapter.client.add_ok_response();
    adapter.client.skip_urc(1);
    adapter.client.add_urc_second_socket_connected();

    adapter
        .connect(&mut socket2, SocketAddr::from_str("127.0.0.1:6000").unwrap())
        .unwrap();

    let commands = adapter.client.get_commands_as_strings();
    assert_eq!(4, commands.len());
    assert_eq!("AT+CIPRECVMODE=1\r\n".to_string(), commands[1]);
    assert_eq!("AT+CIPSTART=0,\"TCP\",\"127.0.0.1\",5000\r\n".to_string(), commands[2]);
    assert_eq!("AT+CIPSTART=1,\"TCP\",\"127.0.0.1\",6000\r\n".to_string(), commands[3]);
}

#[test]
fn test_connect_closing_socket_reconnected() {
    let mut client = MockAtatClient::new();

    // Multiple connections command
    client.add_ok_response();
    // Receiving mode command
    client.add_ok_response();

    let mut adapter = Adapter::new(client);

    let mut socket = adapter.socket().unwrap();

    adapter.client.add_urc_first_socket_closed();
    adapter.process_urc_messages();

    // Connect command
    adapter.client.add_ok_response();
    adapter.client.skip_urc(1);
    adapter.client.add_urc_first_socket_connected();

    adapter
        .connect(&mut socket, SocketAddr::from_str("127.0.0.1:5000").unwrap())
        .unwrap();
}

#[test]
fn test_connect_already_connected_at_second_call() {
    let mut client = MockAtatClient::new();

    // Multiple connections command
    client.add_ok_response();
    // Receiving mode command
    client.add_ok_response();
    // Connect command
    client.add_ok_response();

    client.skip_urc(1);
    client.add_urc_first_socket_connected();

    let mut adapter = Adapter::new(client);

    let mut socket = adapter.socket().unwrap();
    adapter
        .connect(&mut socket, SocketAddr::from_str("127.0.0.1:5000").unwrap())
        .unwrap();

    let error = adapter
        .connect(&mut socket, SocketAddr::from_str("127.0.0.1:6000").unwrap())
        .unwrap_err();

    assert_eq!(nb::Error::Other(Error::AlreadyConnected), error);
}

#[test]
fn test_connect_unconfirmed() {
    let mut client = MockAtatClient::new();

    // Multiple connections command
    client.add_ok_response();
    // Receiving mode command
    client.add_ok_response();
    // Connect command
    client.add_ok_response();

    let mut adapter = Adapter::new(client);

    let mut socket = adapter.socket().unwrap();
    let error = adapter
        .connect(&mut socket, SocketAddr::from_str("127.0.0.1:6000").unwrap())
        .unwrap_err();

    assert_eq!(nb::Error::Other(Error::ConnectUnconfirmed), error);
}
