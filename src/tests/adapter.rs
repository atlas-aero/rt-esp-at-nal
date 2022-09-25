use crate::adapter::{Adapter, JoinError};
use crate::tests::mock::MockAtatClient;
use alloc::string::ToString;
use atat::Error;

#[test]
fn test_join_mode_error() {
    let mut client = MockAtatClient::new();
    client.add_error_response();

    let mut adapter = Adapter::new(client);
    let result = adapter.join("test_wifi", "secret").unwrap_err();

    assert_eq!(JoinError::ModeError(Error::Parse), result);

    let commands = adapter.client.get_commands_as_strings();
    assert_eq!(1, commands.len());
    assert_eq!("AT+CWMODE=1\r\n".to_string(), commands[0]);
}

#[test]
fn test_join_mode_would_block() {
    let mut client = MockAtatClient::new();
    client.send_would_block(0);

    let mut adapter = Adapter::new(client);
    let result = adapter.join("test_wifi", "secret").unwrap_err();

    assert_eq!(JoinError::UnexpectedWouldBlock, result);
}

#[test]
fn test_join_connect_command_error() {
    let mut client = MockAtatClient::new();
    client.add_ok_response();
    client.add_error_response();

    let mut adapter = Adapter::new(client);
    let result = adapter.join("test_wifi", "secret").unwrap_err();

    assert_eq!(JoinError::ConnectError(Error::Parse), result);

    let commands = adapter.client.get_commands_as_strings();
    assert_eq!(2, commands.len());
    assert_eq!("AT+CWJAP=\"test_wifi\",\"secret\"\r\n".to_string(), commands[1]);
}

#[test]
fn test_join_connect_command_would_block() {
    let mut client = MockAtatClient::new();
    client.add_ok_response();
    client.send_would_block(1);

    let mut adapter = Adapter::new(client);
    let result = adapter.join("test_wifi", "secret").unwrap_err();

    assert_eq!(JoinError::UnexpectedWouldBlock, result);

    let commands = adapter.client.get_commands_as_strings();
    assert_eq!(2, commands.len());
    assert_eq!("AT+CWMODE=1\r\n".to_string(), commands[0]);
}

#[test]
fn test_join_correct_commands() {
    let mut client = MockAtatClient::new();
    client.add_ok_response();
    client.add_ok_response();

    let mut adapter = Adapter::new(client);
    let _ = adapter.join("test_wifi", "secret").unwrap();

    let commands = adapter.client.get_commands_as_strings();
    assert_eq!(2, commands.len());
    assert_eq!("AT+CWMODE=1\r\n".to_string(), commands[0]);
    assert_eq!("AT+CWJAP=\"test_wifi\",\"secret\"\r\n".to_string(), commands[1]);
}

#[test]
fn test_join_wifi_connected() {
    let mut client = MockAtatClient::new();
    client.add_ok_response();
    client.add_ok_response();
    client.add_urc_wifi_connected();

    let mut adapter = Adapter::new(client);

    let result = adapter.join("test_wifi", "secret").unwrap();
    assert!(result.connected);
    assert!(!result.ip_assigned);
}

#[test]
fn test_join_wifi_disconnect() {
    let mut client = MockAtatClient::new();
    client.add_ok_response();
    client.add_ok_response();
    client.add_urc_wifi_disconnect();

    let mut adapter = Adapter::new(client);

    let result = adapter.join("test_wifi", "secret").unwrap();
    assert!(!result.connected);
    assert!(!result.ip_assigned);
}

#[test]
fn test_join_wifi_got_ip() {
    let mut client = MockAtatClient::new();
    client.add_ok_response();
    client.add_ok_response();
    client.add_urc_wifi_connected();
    client.add_urc_wifi_got_ip();

    let mut adapter = Adapter::new(client);

    let result = adapter.join("test_wifi", "secret").unwrap();
    assert!(result.connected);
    assert!(result.ip_assigned);
}

#[test]
fn test_join_other_urc_messages_ignored() {
    let mut client = MockAtatClient::new();
    client.add_ok_response();
    client.add_ok_response();
    client.add_urc_ready();
    client.add_urc_wifi_connected();
    client.add_urc_unknown();
    client.add_urc_wifi_got_ip();

    let mut adapter = Adapter::new(client);

    let result = adapter.join("test_wifi", "secret").unwrap();
    assert!(result.connected);
    assert!(result.ip_assigned);
}

#[test]
fn test_join_wifi_no_urc_messages() {
    let mut client = MockAtatClient::new();
    client.add_ok_response();
    client.add_ok_response();

    let mut adapter = Adapter::new(client);

    let result = adapter.join("test_wifi", "secret").unwrap();
    assert!(!result.connected);
    assert!(!result.ip_assigned);
}
