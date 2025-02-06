use crate::tests::mock::{MockAtatClient, MockTimer, MockedCommand};
use crate::urc::URCMessages;
use crate::wifi::{Adapter, JoinError};
use crate::wifi::{CommandError, WifiAdapter};
use atat::Error;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::pubsub::PubSubChannel;

type AdapterType<'a> = Adapter<'a, MockAtatClient<'a>, MockTimer, 1_000_000, 32, 16, 16>;

#[test]
fn test_join_mode_error() {
    let channel: PubSubChannel<CriticalSectionRawMutex, URCMessages<16>, 16, 1, 1> = PubSubChannel::new();
    let mut client = MockAtatClient::new(&channel);
    let timer = MockTimer::new();
    client.add_response(MockedCommand::error(Some(b"AT+CWMODE=1\r\n"), None));

    let mut adapter: AdapterType = Adapter::new(client, channel.subscriber().unwrap(), timer);
    let result = adapter.join("test_wifi", "secret").unwrap_err();

    assert_eq!(JoinError::ModeError(Error::Parse), result);
}

#[test]
fn test_join_connect_command_error() {
    let channel: PubSubChannel<CriticalSectionRawMutex, URCMessages<16>, 16, 1, 1> = PubSubChannel::new();
    let mut client = MockAtatClient::new(&channel);
    let timer = MockTimer::new();

    client.add_response(MockedCommand::ok(Some(b"AT+CWMODE=1\r\n"), None));
    client.add_response(MockedCommand::error(
        Some(b"AT+CWJAP=\"test_wifi\",\"secret\"\r\n"),
        None,
    ));

    let mut adapter: AdapterType = Adapter::new(client, channel.subscriber().unwrap(), timer);
    let result = adapter.join("test_wifi", "secret").unwrap_err();

    assert_eq!(JoinError::ConnectError(Error::Parse), result);
}

#[test]
fn test_join_correct_commands() {
    let timer = MockTimer::new();
    let channel: PubSubChannel<CriticalSectionRawMutex, URCMessages<16>, 16, 1, 1> = PubSubChannel::new();
    let mut client = MockAtatClient::new(&channel);

    client.add_response(MockedCommand::ok(Some(b"AT+CWMODE=1\r\n"), None));
    client.add_response(MockedCommand::ok(Some(b"AT+CWJAP=\"test_wifi\",\"secret\"\r\n"), None));

    let mut adapter: AdapterType = Adapter::new(client, channel.subscriber().unwrap(), timer);
    let _ = adapter.join("test_wifi", "secret").unwrap();
    adapter.client.assert_all_cmds_sent();
}

#[test]
fn test_join_wifi_connected() {
    let timer = MockTimer::new();
    let channel: PubSubChannel<CriticalSectionRawMutex, URCMessages<16>, 16, 1, 1> = PubSubChannel::new();
    let mut client = MockAtatClient::new(&channel);
    client.add_response(MockedCommand::ok(Some(b"AT+CWMODE=1\r\n"), None));
    client.add_response(MockedCommand::ok(
        Some(b"AT+CWJAP=\"test_wifi\",\"secret\"\r\n"),
        Some(&[b"WIFI CONNECTED\r\n"]),
    ));
    client.add_urc_wifi_connected();

    let mut adapter: AdapterType = Adapter::new(client, channel.subscriber().unwrap(), timer);

    let result = adapter.join("test_wifi", "secret").unwrap();
    assert!(result.connected);
    assert!(!result.ip_assigned);
    adapter.client.assert_all_cmds_sent();
}

#[test]
fn test_join_wifi_disconnect() {
    let timer = MockTimer::new();
    let channel: PubSubChannel<CriticalSectionRawMutex, URCMessages<16>, 16, 1, 1> = PubSubChannel::new();
    let mut client = MockAtatClient::new(&channel);
    client.add_response(MockedCommand::ok(Some(b"AT+CWMODE=1\r\n"), None));
    client.add_response(MockedCommand::ok(
        Some(b"AT+CWJAP=\"test_wifi\",\"secret\"\r\n"),
        Some(&[b"WIFI CONNECTED\r\n", b"WIFI DISCONNECT\r\n"]),
    ));

    let mut adapter: AdapterType = Adapter::new(client, channel.subscriber().unwrap(), timer);

    let result = adapter.join("test_wifi", "secret").unwrap();
    assert!(!result.connected);
    assert!(!result.ip_assigned);
    adapter.client.assert_all_cmds_sent();
}

#[test]
fn test_join_wifi_got_ip() {
    let timer = MockTimer::new();
    let channel: PubSubChannel<CriticalSectionRawMutex, URCMessages<16>, 16, 1, 1> = PubSubChannel::new();
    let mut client = MockAtatClient::new(&channel);

    client.add_response(MockedCommand::ok(Some(b"AT+CWMODE=1\r\n"), None));
    client.add_response(MockedCommand::ok(
        Some(b"AT+CWJAP=\"test_wifi\",\"secret\"\r\n"),
        Some(&[b"WIFI CONNECTED\r\n", b"WIFI GOT IP\r\n"]),
    ));

    let mut adapter: AdapterType = Adapter::new(client, channel.subscriber().unwrap(), timer);

    let result = adapter.join("test_wifi", "secret").unwrap();
    assert!(result.connected);
    assert!(result.ip_assigned);
    adapter.client.assert_all_cmds_sent();
}

#[test]
fn test_join_other_urc_messages_ignored() {
    let timer = MockTimer::new();
    let channel: PubSubChannel<CriticalSectionRawMutex, URCMessages<16>, 16, 1, 1> = PubSubChannel::new();
    let mut client = MockAtatClient::new(&channel);

    client.add_response(MockedCommand::ok(Some(b"AT+CWMODE=1\r\n"), None));
    client.add_response(MockedCommand::ok(
        Some(b"AT+CWJAP=\"test_wifi\",\"secret\"\r\n"),
        Some(&[b"WIFI CONNECTED\r\n", b"ready\r\n", b"UNKNOWN\r\n", b"WIFI GOT IP\r\n"]),
    ));

    let mut adapter: AdapterType = Adapter::new(client, channel.subscriber().unwrap(), timer);

    let result = adapter.join("test_wifi", "secret").unwrap();
    assert!(result.connected);
    assert!(result.ip_assigned);
    adapter.client.assert_all_cmds_sent();
}

#[test]
fn test_join_wifi_no_urc_messages() {
    let timer = MockTimer::new();
    let channel: PubSubChannel<CriticalSectionRawMutex, URCMessages<16>, 16, 1, 1> = PubSubChannel::new();
    let mut client = MockAtatClient::new(&channel);

    client.add_response(MockedCommand::ok(Some(b"AT+CWMODE=1\r\n"), None));
    client.add_response(MockedCommand::ok(Some(b"AT+CWJAP=\"test_wifi\",\"secret\"\r\n"), None));

    let mut adapter: AdapterType = Adapter::new(client, channel.subscriber().unwrap(), timer);

    let result = adapter.join("test_wifi", "secret").unwrap();
    assert!(!result.connected);
    assert!(!result.ip_assigned);
    adapter.client.assert_all_cmds_sent();
}

#[test]
fn test_get_join_state_disconnected() {
    let timer = MockTimer::new();
    let channel: PubSubChannel<CriticalSectionRawMutex, URCMessages<16>, 16, 1, 1> = PubSubChannel::new();
    let mut client = MockAtatClient::new(&channel);
    // Simulate that network was connected once
    client.add_urc_wifi_connected();
    client.add_urc_wifi_got_ip();

    client.add_urc_wifi_disconnect();
    let mut adapter: AdapterType = Adapter::new(client, channel.subscriber().unwrap(), timer);

    let result = adapter.get_join_status();
    assert!(!result.connected);
    assert!(!result.ip_assigned);
}

#[test]
fn test_get_join_state_connected_and_ip_assigned() {
    let timer = MockTimer::new();
    let channel: PubSubChannel<CriticalSectionRawMutex, URCMessages<16>, 16, 1, 1> = PubSubChannel::new();
    let client = MockAtatClient::new(&channel);
    let mut adapter: AdapterType = Adapter::new(client, channel.subscriber().unwrap(), timer);

    adapter.client.add_urc_wifi_connected();
    adapter.client.add_urc_wifi_got_ip();

    let result = adapter.get_join_status();
    assert!(result.connected);
    assert!(result.ip_assigned);
}

#[test]
fn test_get_join_state_connected_without_ip() {
    let timer = MockTimer::new();
    let channel: PubSubChannel<CriticalSectionRawMutex, URCMessages<16>, 16, 1, 1> = PubSubChannel::new();
    let client = MockAtatClient::new(&channel);
    let mut adapter: AdapterType = Adapter::new(client, channel.subscriber().unwrap(), timer);

    adapter.client.add_urc_wifi_connected();

    let result = adapter.get_join_status();
    assert!(result.connected);
    assert!(!result.ip_assigned);
}

#[test]
fn test_restart_command_failed() {
    let channel: PubSubChannel<CriticalSectionRawMutex, URCMessages<16>, 16, 1, 1> = PubSubChannel::new();
    let mut client = MockAtatClient::new(&channel);
    let timer = MockTimer::new();
    client.add_response(MockedCommand::error(Some(b"AT+RST\r\n"), None));

    let mut adapter: AdapterType = Adapter::new(client, channel.subscriber().unwrap(), timer);
    let error = adapter.restart().unwrap_err();

    assert_eq!(CommandError::CommandFailed(Error::Parse), error);
}

#[test]
fn test_restart_upstream_timer_start_error() {
    let channel: PubSubChannel<CriticalSectionRawMutex, URCMessages<16>, 16, 1, 1> = PubSubChannel::new();
    let mut client = MockAtatClient::new(&channel);
    client.add_response(MockedCommand::ok(Some(b"AT+RST\r\n"), None));

    let mut timer = MockTimer::new();
    timer.expect_start().times(1).returning(move |_| Err(31));

    let mut adapter: AdapterType = Adapter::new(client, channel.subscriber().unwrap(), timer);
    let error = adapter.restart().unwrap_err();

    assert_eq!(CommandError::TimerError, error);
}

#[test]
fn test_restart_upstream_timer_wait_error() {
    let channel: PubSubChannel<CriticalSectionRawMutex, URCMessages<16>, 16, 1, 1> = PubSubChannel::new();
    let mut client = MockAtatClient::new(&channel);
    client.add_response(MockedCommand::ok(Some(b"AT+RST\r\n"), None));

    let mut timer = MockTimer::new();
    timer.expect_start().times(1).returning(move |_| Ok(()));
    timer
        .expect_wait()
        .times(1)
        .returning(move || nb::Result::Err(nb::Error::Other(1)));

    let mut adapter: AdapterType = Adapter::new(client, channel.subscriber().unwrap(), timer);
    let error = adapter.restart().unwrap_err();

    assert_eq!(CommandError::TimerError, error);
}

#[test]
fn test_restart_ready_received() {
    let channel: PubSubChannel<CriticalSectionRawMutex, URCMessages<16>, 16, 1, 1> = PubSubChannel::new();
    let client = MockAtatClient::new(&channel);

    let mut timer = MockTimer::new();
    timer.expect_start().times(1).returning(|duration| {
        assert_eq!(duration, MockTimer::duration_ms(5_000));
        Ok(())
    });
    timer
        .expect_wait()
        .times(1)
        .returning(|| nb::Result::Err(nb::Error::WouldBlock));

    let mut adapter: AdapterType = Adapter::new(client, channel.subscriber().unwrap(), timer);

    adapter.client.add_response(MockedCommand::ok(Some(b"AT+RST\r\n"), None));
    adapter.client.add_urc_ready();

    adapter.restart().unwrap();
}

#[test]
fn test_restart_double() {
    let channel: PubSubChannel<CriticalSectionRawMutex, URCMessages<16>, 16, 1, 1> = PubSubChannel::new();
    let mut client = MockAtatClient::new(&channel);
    client.add_response(MockedCommand::ok(Some(b"AT+RST\r\n"), Some(&[b"ready\r\n"])));

    let mut timer = MockTimer::new();
    timer.expect_start().times(2).returning(|duration| {
        assert_eq!(duration, MockTimer::duration_ms(5_000));
        Ok(())
    });
    timer
        .expect_wait()
        .times(2)
        .returning(|| nb::Result::Err(nb::Error::WouldBlock));

    let mut adapter: AdapterType = Adapter::new(client, channel.subscriber().unwrap(), timer);
    adapter.restart().unwrap();

    adapter
        .client
        .add_response(MockedCommand::ok(Some(b"AT+RST\r\n"), Some(&[b"ready\r\n"])));

    // Assert that ready state is reset and a second restart is possible
    adapter.restart().unwrap();
    adapter.client.assert_all_cmds_sent();
}

#[test]
fn test_restart_ready_timeout() {
    let channel: PubSubChannel<CriticalSectionRawMutex, URCMessages<16>, 16, 1, 1> = PubSubChannel::new();
    let mut client = MockAtatClient::new(&channel);
    client.add_response(MockedCommand::ok(Some(b"AT+RST\r\n"), None));

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

    let mut adapter: AdapterType = Adapter::new(client, channel.subscriber().unwrap(), timer);
    let error = adapter.restart().unwrap_err();

    assert_eq!(CommandError::ReadyTimeout, error);
}

#[test]
fn test_restart_wifi_state_reset() {
    let channel: PubSubChannel<CriticalSectionRawMutex, URCMessages<16>, 16, 1, 1> = PubSubChannel::new();
    let mut client = MockAtatClient::new(&channel);
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

    let mut adapter: AdapterType = Adapter::new(client, channel.subscriber().unwrap(), timer);
    // Faking WIFI connection state
    adapter.process_urc_messages();

    adapter
        .client
        .add_response(MockedCommand::ok(Some(b"AT+RST\r\n"), Some(&[b"ready\r\n"])));
    adapter.restart().unwrap();

    assert!(!adapter.get_join_status().connected);
    assert!(!adapter.get_join_status().ip_assigned);
}

#[test]
fn test_set_auto_connect_error() {
    let channel: PubSubChannel<CriticalSectionRawMutex, URCMessages<16>, 16, 1, 1> = PubSubChannel::new();
    let mut client = MockAtatClient::new(&channel);
    let timer = MockTimer::new();
    client.add_response(MockedCommand::error(Some(b"AT+CWAUTOCONN=1\r\n"), None));

    let mut adapter: AdapterType = Adapter::new(client, channel.subscriber().unwrap(), timer);
    let result = adapter.set_auto_connect(true).unwrap_err();

    assert_eq!(CommandError::CommandFailed(Error::Parse), result);
}

#[test]
fn test_set_auto_connect_correct_command() {
    let channel: PubSubChannel<CriticalSectionRawMutex, URCMessages<16>, 16, 1, 1> = PubSubChannel::new();
    let mut client = MockAtatClient::new(&channel);
    let timer = MockTimer::new();

    client.add_response(MockedCommand::ok(Some(b"AT+CWAUTOCONN=1\r\n"), None));
    client.add_response(MockedCommand::ok(Some(b"AT+CWAUTOCONN=0\r\n"), None));

    let mut adapter: AdapterType = Adapter::new(client, channel.subscriber().unwrap(), timer);
    adapter.set_auto_connect(true).unwrap();
    adapter.set_auto_connect(false).unwrap();
    adapter.client.assert_all_cmds_sent();
}
