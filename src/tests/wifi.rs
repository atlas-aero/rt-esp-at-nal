use crate::tests::mock::{MockAtatClient, MockTimer};
use crate::wifi::{Adapter, JoinError};
use crate::wifi::{CommandError, WifiAdapter};
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

#[test]
fn test_restart_command_failed() {
    let mut client = MockAtatClient::new();
    let timer = MockTimer::new();
    client.add_error_response();

    let mut adapter: AdapterType = Adapter::new(client, timer);
    let error = adapter.restart().unwrap_err();

    assert_eq!(CommandError::CommandFailed(Error::Parse), error);
}

#[test]
fn test_restart_command_would_block() {
    let mut client = MockAtatClient::new();
    let timer = MockTimer::new();
    client.send_would_block(0);

    let mut adapter: AdapterType = Adapter::new(client, timer);
    let error = adapter.restart().unwrap_err();

    assert_eq!(CommandError::UnexpectedWouldBlock, error);
}

#[test]
fn test_restart_upstream_timer_start_error() {
    let mut client = MockAtatClient::new();
    client.add_ok_response();

    let mut timer = MockTimer::new();
    timer.expect_start().times(1).returning(move |_| Err(31));

    let mut adapter: AdapterType = Adapter::new(client, timer);
    let error = adapter.restart().unwrap_err();

    assert_eq!(CommandError::TimerError, error);
}

#[test]
fn test_restart_upstream_timer_wait_error() {
    let mut client = MockAtatClient::new();
    client.add_ok_response();

    let mut timer = MockTimer::new();
    timer.expect_start().times(1).returning(move |_| Ok(()));
    timer
        .expect_wait()
        .times(1)
        .returning(move || nb::Result::Err(nb::Error::Other(1)));

    let mut adapter: AdapterType = Adapter::new(client, timer);
    let error = adapter.restart().unwrap_err();

    assert_eq!(CommandError::TimerError, error);
}

#[test]
fn test_restart_ready_received() {
    let mut client = MockAtatClient::new();
    client.add_ok_response();
    client.add_urc_ready();

    let mut timer = MockTimer::new();
    timer.expect_start().times(1).returning(|duration| {
        assert_eq!(duration, MockTimer::duration_ms(5_000));
        Ok(())
    });
    timer
        .expect_wait()
        .times(1)
        .returning(|| nb::Result::Err(nb::Error::WouldBlock));

    let mut adapter: AdapterType = Adapter::new(client, timer);
    adapter.restart().unwrap();

    let commands = adapter.client.get_commands_as_strings();
    assert_eq!(1, commands.len());
    assert_eq!("AT+RST\r\n".to_string(), commands[0]);
}

#[test]
fn test_restart_double() {
    let mut client = MockAtatClient::new();
    client.add_ok_response();
    client.add_urc_ready();

    let mut timer = MockTimer::new();
    timer.expect_start().times(2).returning(|duration| {
        assert_eq!(duration, MockTimer::duration_ms(5_000));
        Ok(())
    });
    timer
        .expect_wait()
        .times(2)
        .returning(|| nb::Result::Err(nb::Error::WouldBlock));

    let mut adapter: AdapterType = Adapter::new(client, timer);
    adapter.restart().unwrap();

    adapter.client.add_ok_response();
    adapter.client.add_urc_ready();

    // Assert that ready state is reset and a second restart is possible
    adapter.restart().unwrap();

    assert_eq!(2, adapter.client.get_commands_as_strings().len());
}

#[test]
fn test_restart_ready_timeout() {
    let mut client = MockAtatClient::new();
    client.add_ok_response();

    let mut timer = MockTimer::new();
    timer.expect_start().times(1).returning(|duration| {
        assert_eq!(duration, MockTimer::duration_ms(5_000));
        Ok(())
    });
    timer
        .expect_wait()
        .times(1)
        .returning(|| nb::Result::Err(nb::Error::WouldBlock));
    timer.expect_wait().times(1).returning(|| nb::Result::Ok(()));

    let mut adapter: AdapterType = Adapter::new(client, timer);
    let error = adapter.restart().unwrap_err();

    assert_eq!(CommandError::ReadyTimeout, error);
}

#[test]
fn test_restart_wifi_state_reset() {
    let mut client = MockAtatClient::new();
    client.add_urc_wifi_connected();
    client.add_urc_wifi_got_ip();

    let mut timer = MockTimer::new();
    timer.expect_start().times(1).returning(|duration| {
        assert_eq!(duration, MockTimer::duration_ms(5_000));
        Ok(())
    });
    timer
        .expect_wait()
        .times(1)
        .returning(|| nb::Result::Err(nb::Error::WouldBlock));

    let mut adapter: AdapterType = Adapter::new(client, timer);
    // Faking WIFI connection state
    adapter.process_urc_messages();

    adapter.client.add_ok_response();
    adapter.client.add_urc_ready();
    adapter.restart().unwrap();

    assert!(!adapter.get_join_status().connected);
    assert!(!adapter.get_join_status().ip_assigned);
}

#[test]
fn test_set_auto_connect_error() {
    let mut client = MockAtatClient::new();
    let timer = MockTimer::new();
    client.add_error_response();

    let mut adapter: AdapterType = Adapter::new(client, timer);
    let result = adapter.set_auto_connect(true).unwrap_err();

    assert_eq!(CommandError::CommandFailed(Error::Parse), result);
}

#[test]
fn test_enable_auto_connect_would_block() {
    let mut client = MockAtatClient::new();
    let timer = MockTimer::new();
    client.send_would_block(0);

    let mut adapter: AdapterType = Adapter::new(client, timer);
    let error = adapter.set_auto_connect(true).unwrap_err();

    assert_eq!(CommandError::UnexpectedWouldBlock, error);
}

#[test]
fn test_set_auto_connect_correct_command() {
    let mut client = MockAtatClient::new();
    let timer = MockTimer::new();

    client.add_ok_response();
    client.add_ok_response();

    let mut adapter: AdapterType = Adapter::new(client, timer);
    adapter.set_auto_connect(true).unwrap();
    adapter.set_auto_connect(false).unwrap();

    let commands = adapter.client.get_commands_as_strings();
    assert_eq!(2, commands.len());
    assert_eq!("AT+CWAUTOCONN=1\r\n".to_string(), commands[0]);
    assert_eq!("AT+CWAUTOCONN=0\r\n".to_string(), commands[1]);
}
