use crate::tests::mock::{MockAtatClient, MockTimer};
use crate::wifi::WifiAdapter;
use crate::wifi::{Adapter, JoinError};
use alloc::string::ToString;
use atat::Error;

type AdapterType = Adapter<MockAtatClient, MockTimer, 1_000_000, 256, 64>;

#[test]
fn test_join_mode_error() {
    let mut client = MockAtatClient::new();
    let timer = MockTimer::new();
    client.add_error_response();

    let mut adapter: AdapterType = Adapter::new(client, timer);
    let result = adapter.join("test_wifi", "secret").unwrap_err();

    assert_eq!(JoinError::ModeError(Error::Parse), result);

    let commands = adapter.client.get_commands_as_strings();
    assert_eq!(1, commands.len());
    assert_eq!("AT+CWMODE=1\r\n".to_string(), commands[0]);
}

#[test]
fn test_join_mode_would_block() {
    let mut client = MockAtatClient::new();
    let timer = MockTimer::new();
    client.send_would_block(0);

    let mut adapter: AdapterType = Adapter::new(client, timer);
    let result = adapter.join("test_wifi", "secret").unwrap_err();

    assert_eq!(JoinError::UnexpectedWouldBlock, result);
}

#[test]
fn test_join_connect_command_error() {
    let mut client = MockAtatClient::new();
    let timer = MockTimer::new();

    client.add_ok_response();
    client.add_error_response();

    let mut adapter: AdapterType = Adapter::new(client, timer);
    let result = adapter.join("test_wifi", "secret").unwrap_err();

    assert_eq!(JoinError::ConnectError(Error::Parse), result);

    let commands = adapter.client.get_commands_as_strings();
    assert_eq!(2, commands.len());
    assert_eq!("AT+CWJAP=\"test_wifi\",\"secret\"\r\n".to_string(), commands[1]);
}

#[test]
fn test_join_connect_command_would_block() {
    let timer = MockTimer::new();
    let mut client = MockAtatClient::new();

    client.add_ok_response();
    client.send_would_block(1);

    let mut adapter: AdapterType = Adapter::new(client, timer);
    let result = adapter.join("test_wifi", "secret").unwrap_err();

    assert_eq!(JoinError::UnexpectedWouldBlock, result);

    let commands = adapter.client.get_commands_as_strings();
    assert_eq!(2, commands.len());
    assert_eq!("AT+CWMODE=1\r\n".to_string(), commands[0]);
}

#[test]
fn test_join_correct_commands() {
    let timer = MockTimer::new();
    let mut client = MockAtatClient::new();

    client.add_ok_response();
    client.add_ok_response();

    let mut adapter: AdapterType = Adapter::new(client, timer);
    let _ = adapter.join("test_wifi", "secret").unwrap();

    let commands = adapter.client.get_commands_as_strings();
    assert_eq!(2, commands.len());
    assert_eq!("AT+CWMODE=1\r\n".to_string(), commands[0]);
    assert_eq!("AT+CWJAP=\"test_wifi\",\"secret\"\r\n".to_string(), commands[1]);
}

#[test]
fn test_join_wifi_connected() {
    let timer = MockTimer::new();
    let mut client = MockAtatClient::new();
    client.add_ok_response();
    client.add_ok_response();
    client.add_urc_wifi_connected();

    let mut adapter: AdapterType = Adapter::new(client, timer);

    let result = adapter.join("test_wifi", "secret").unwrap();
    assert!(result.connected);
    assert!(!result.ip_assigned);
}

#[test]
fn test_join_wifi_disconnect() {
    let timer = MockTimer::new();
    let mut client = MockAtatClient::new();
    client.add_ok_response();
    client.add_ok_response();
    client.add_urc_wifi_disconnect();

    let mut adapter: AdapterType = Adapter::new(client, timer);

    let result = adapter.join("test_wifi", "secret").unwrap();
    assert!(!result.connected);
    assert!(!result.ip_assigned);
}

#[test]
fn test_join_wifi_got_ip() {
    let timer = MockTimer::new();
    let mut client = MockAtatClient::new();
    client.add_ok_response();
    client.add_ok_response();
    client.add_urc_wifi_connected();
    client.add_urc_wifi_got_ip();

    let mut adapter: AdapterType = Adapter::new(client, timer);

    let result = adapter.join("test_wifi", "secret").unwrap();
    assert!(result.connected);
    assert!(result.ip_assigned);
}

#[test]
fn test_join_other_urc_messages_ignored() {
    let timer = MockTimer::new();
    let mut client = MockAtatClient::new();
    client.add_ok_response();
    client.add_ok_response();
    client.add_urc_ready();
    client.add_urc_wifi_connected();
    client.add_urc_unknown();
    client.add_urc_wifi_got_ip();

    let mut adapter: AdapterType = Adapter::new(client, timer);

    let result = adapter.join("test_wifi", "secret").unwrap();
    assert!(result.connected);
    assert!(result.ip_assigned);
}

#[test]
fn test_join_wifi_no_urc_messages() {
    let timer = MockTimer::new();
    let mut client = MockAtatClient::new();
    client.add_ok_response();
    client.add_ok_response();

    let mut adapter: AdapterType = Adapter::new(client, timer);

    let result = adapter.join("test_wifi", "secret").unwrap();
    assert!(!result.connected);
    assert!(!result.ip_assigned);
}

#[test]
fn test_get_join_state_disconnected() {
    let timer = MockTimer::new();
    let mut client = MockAtatClient::new();
    // Simulate that network was connected once
    client.add_urc_wifi_connected();
    client.add_urc_wifi_got_ip();

    client.add_urc_wifi_disconnect();
    let mut adapter: AdapterType = Adapter::new(client, timer);

    let result = adapter.get_join_status();
    assert!(!result.connected);
    assert!(!result.ip_assigned);
}

#[test]
fn test_get_join_state_connected_and_ip_assigned() {
    let timer = MockTimer::new();
    let mut client = MockAtatClient::new();
    client.add_urc_wifi_connected();
    client.add_urc_wifi_got_ip();

    let mut adapter: AdapterType = Adapter::new(client, timer);

    let result = adapter.get_join_status();
    assert!(result.connected);
    assert!(result.ip_assigned);
}

#[test]
fn test_get_join_state_connected_without_ip() {
    let timer = MockTimer::new();
    let mut client = MockAtatClient::new();
    client.add_urc_wifi_connected();

    let mut adapter: AdapterType = Adapter::new(client, timer);

    let result = adapter.get_join_status();
    assert!(result.connected);
    assert!(!result.ip_assigned);
}
