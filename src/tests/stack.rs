use crate::stack::{Error, Socket};
use crate::tests::mock::{MockAtatClient, MockTimer, MockedCommand};
use crate::urc::URCMessages;
use crate::wifi::{Adapter, WifiAdapter};
use alloc::vec;
use atat::Error as AtError;
use core::net::SocketAddr;
use core::str::FromStr;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::pubsub::PubSubChannel;
use embedded_nal::TcpClientStack;

type AdapterType<'a> = Adapter<'a, MockAtatClient<'a>, MockTimer, 1_000_000, 32, 16, 16>;

#[test]
fn test_socket_multi_conn_error() {
    let timer = MockTimer::new();
    let channel: PubSubChannel<CriticalSectionRawMutex, URCMessages<16>, 16, 1, 1> = PubSubChannel::new();
    let mut client = MockAtatClient::new(&channel);
    client.add_response(MockedCommand::error(Some(b"AT+CIPMUX=1\r\n"), None));

    let mut adapter: AdapterType = Adapter::new(client, channel.subscriber().unwrap(), timer);
    let result = adapter.socket().unwrap_err();
    assert_eq!(Error::EnablingMultiConnectionsFailed(AtError::Parse), result);
}

#[test]
fn test_socket_multi_conn_enabled_once() {
    let timer = MockTimer::new();
    let channel: PubSubChannel<CriticalSectionRawMutex, URCMessages<16>, 16, 1, 1> = PubSubChannel::new();
    let mut client = MockAtatClient::new(&channel);
    client.add_response(MockedCommand::ok(Some(b"AT+CIPMUX=1\r\n"), None));

    let mut adapter: AdapterType = Adapter::new(client, channel.subscriber().unwrap(), timer);
    adapter.socket().unwrap();
    adapter.socket().unwrap();
    adapter.client.assert_all_cmds_sent();
}

#[test]
fn test_socket_opened() {
    let timer = MockTimer::new();
    let channel: PubSubChannel<CriticalSectionRawMutex, URCMessages<16>, 16, 1, 1> = PubSubChannel::new();
    let mut client = MockAtatClient::new(&channel);
    client.add_response(MockedCommand::ok(Some(b"AT+CIPMUX=1\r\n"), None));

    let mut adapter: AdapterType = Adapter::new(client, channel.subscriber().unwrap(), timer);
    assert_eq!(0, adapter.socket().unwrap().link_id);
    assert_eq!(1, adapter.socket().unwrap().link_id);
    assert_eq!(2, adapter.socket().unwrap().link_id);
    assert_eq!(3, adapter.socket().unwrap().link_id);
    assert_eq!(4, adapter.socket().unwrap().link_id);
    adapter.client.assert_all_cmds_sent();
}

#[test]
fn test_socket_not_available() {
    let timer = MockTimer::new();
    let channel: PubSubChannel<CriticalSectionRawMutex, URCMessages<16>, 16, 1, 1> = PubSubChannel::new();
    let mut client = MockAtatClient::new(&channel);
    client.add_response(MockedCommand::ok(Some(b"AT+CIPMUX=1\r\n"), None));

    let mut adapter: AdapterType = Adapter::new(client, channel.subscriber().unwrap(), timer);
    for _ in 0..5 {
        adapter.socket().unwrap();
    }

    let result = adapter.socket().unwrap_err();
    assert_eq!(Error::NoSocketAvailable, result);
}

#[test]
fn test_connect_already_connected_by_urc() {
    let timer = MockTimer::new();
    let channel: PubSubChannel<CriticalSectionRawMutex, URCMessages<16>, 16, 1, 1> = PubSubChannel::new();
    let mut client = MockAtatClient::new(&channel);

    // Multiple connections command
    client.add_response(MockedCommand::ok(Some(b"AT+CIPMUX=1\r\n"), None));

    let mut adapter: AdapterType = Adapter::new(client, channel.subscriber().unwrap(), timer);
    adapter.client.add_urc_first_socket_connected();

    let mut socket = adapter.socket().unwrap();
    let error = adapter
        .connect(&mut socket, SocketAddr::from_str("127.0.0.1:5000").unwrap())
        .unwrap_err();

    assert_eq!(nb::Error::Other(Error::AlreadyConnected), error);
}

#[test]
fn test_connect_already_connected_by_response() {
    let timer = MockTimer::new();
    let channel: PubSubChannel<CriticalSectionRawMutex, URCMessages<16>, 16, 1, 1> = PubSubChannel::new();
    let mut client = MockAtatClient::new(&channel);

    // Multiple connections command
    client.add_response(MockedCommand::ok(Some(b"AT+CIPMUX=1\r\n"), None));

    // Receiving mode command
    client.add_response(MockedCommand::ok(Some(b"AT+CIPRECVMODE=1\r\n"), None));

    // Connect command
    client.add_response(MockedCommand::ok(
        Some(b"AT+CIPSTART=0,\"TCP\",\"127.0.0.1\",5000\r\n"),
        Some(&[b"ALREADY CONNECTED\r\n"]),
    ));

    let mut adapter: AdapterType = Adapter::new(client, channel.subscriber().unwrap(), timer);

    let mut socket = adapter.socket().unwrap();
    adapter
        .connect(&mut socket, SocketAddr::from_str("127.0.0.1:5000").unwrap())
        .unwrap();

    assert!(adapter.is_connected(&socket).unwrap());
    adapter.client.assert_all_cmds_sent();
}

#[test]
fn test_connect_correct_commands_ipv4() {
    let timer = MockTimer::new();
    let channel: PubSubChannel<CriticalSectionRawMutex, URCMessages<16>, 16, 1, 1> = PubSubChannel::new();
    let mut client = MockAtatClient::new(&channel);

    // Multiple connections command
    client.add_response(MockedCommand::ok(Some(b"AT+CIPMUX=1\r\n"), None));

    // Receiving mode command
    client.add_response(MockedCommand::ok(Some(b"AT+CIPRECVMODE=1\r\n"), None));

    // Connect command
    client.add_response(MockedCommand::ok(
        Some(b"AT+CIPSTART=0,\"TCP\",\"127.0.0.1\",5000\r\n"),
        Some(&[b"0,CONNECT\r\n"]),
    ));

    let mut adapter: AdapterType = Adapter::new(client, channel.subscriber().unwrap(), timer);

    let mut socket = adapter.socket().unwrap();
    adapter
        .connect(&mut socket, SocketAddr::from_str("127.0.0.1:5000").unwrap())
        .unwrap();
    adapter.client.assert_all_cmds_sent();
}

#[test]
fn test_connect_after_restart() {
    let mut timer = MockTimer::new();
    timer.expect_start().times(1).returning(|_| Ok(()));
    timer
        .expect_wait()
        .times(1)
        .returning(|| nb::Result::Err(nb::Error::WouldBlock));

    let channel: PubSubChannel<CriticalSectionRawMutex, URCMessages<16>, 16, 1, 1> = PubSubChannel::new();
    let mut client = MockAtatClient::new(&channel);

    // Socket commands
    client.add_response(MockedCommand::ok(Some(b"AT+CIPMUX=1\r\n"), None));
    client.add_response(MockedCommand::ok(Some(b"AT+CIPRECVMODE=1\r\n"), None));
    client.add_response(MockedCommand::ok(
        Some(b"AT+CIPSTART=0,\"TCP\",\"127.0.0.1\",5000\r\n"),
        Some(&[b"0,CONNECT\r\n"]),
    ));

    let mut adapter: AdapterType = Adapter::new(client, channel.subscriber().unwrap(), timer);

    let mut socket = adapter.socket().unwrap();
    adapter
        .connect(&mut socket, SocketAddr::from_str("127.0.0.1:5000").unwrap())
        .unwrap();

    // RST command
    adapter
        .client
        .add_response(MockedCommand::ok(Some(b"AT+RST\r\n"), Some(&[b"ready\r\n"])));
    adapter.restart().unwrap();

    // Socket commands
    adapter.client.add_response(MockedCommand::ok(Some(b"AT+CIPMUX=1\r\n"), None));
    adapter
        .client
        .add_response(MockedCommand::ok(Some(b"AT+CIPRECVMODE=1\r\n"), None));
    adapter.client.add_response(MockedCommand::ok(
        Some(b"AT+CIPSTART=0,\"TCP\",\"127.0.0.1\",5000\r\n"),
        Some(&[b"0,CONNECT\r\n"]),
    ));

    socket = adapter.socket().unwrap();
    adapter
        .connect(&mut socket, SocketAddr::from_str("127.0.0.1:5000").unwrap())
        .unwrap();

    // Assert that socket state gets reset on restart
    assert_eq!(0, socket.link_id);
    adapter.client.assert_all_cmds_sent();
}

#[test]
fn test_connect_correct_commands_ipv6() {
    let timer = MockTimer::new();
    let channel: PubSubChannel<CriticalSectionRawMutex, URCMessages<16>, 16, 1, 1> = PubSubChannel::new();
    let mut client = MockAtatClient::new(&channel);

    // Socket commands
    client.add_response(MockedCommand::ok(Some(b"AT+CIPMUX=1\r\n"), None));
    client.add_response(MockedCommand::ok(Some(b"AT+CIPRECVMODE=1\r\n"), None));
    client.add_response(MockedCommand::ok(
        Some(b"AT+CIPSTART=0,\"TCPv6\",\"2001:0db8:0:0:0:0:0:0001\",8080\r\n"),
        Some(&[b"0,CONNECT\r\n"]),
    ));

    let mut adapter: AdapterType = Adapter::new(client, channel.subscriber().unwrap(), timer);

    let mut socket = adapter.socket().unwrap();
    adapter
        .connect(&mut socket, SocketAddr::from_str("[2001:db8::1]:8080").unwrap())
        .unwrap();
    adapter.client.assert_all_cmds_sent();
}

#[test]
fn test_connect_receive_mode_error() {
    let timer = MockTimer::new();
    let channel: PubSubChannel<CriticalSectionRawMutex, URCMessages<16>, 16, 1, 1> = PubSubChannel::new();
    let mut client = MockAtatClient::new(&channel);

    client.add_response(MockedCommand::ok(Some(b"AT+CIPMUX=1\r\n"), None));
    client.add_response(MockedCommand::error(Some(b"AT+CIPRECVMODE=1\r\n"), None));

    let mut adapter: AdapterType = Adapter::new(client, channel.subscriber().unwrap(), timer);

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
fn test_connect_connect_command_error() {
    let timer = MockTimer::new();
    let channel: PubSubChannel<CriticalSectionRawMutex, URCMessages<16>, 16, 1, 1> = PubSubChannel::new();
    let mut client = MockAtatClient::new(&channel);

    client.add_response(MockedCommand::ok(Some(b"AT+CIPMUX=1\r\n"), None));
    client.add_response(MockedCommand::ok(Some(b"AT+CIPRECVMODE=1\r\n"), None));
    client.add_response(MockedCommand::error(
        Some(b"AT+CIPSTART=0,\"TCP\",\"127.0.0.1\",5000\r\n"),
        Some(&[b"0,CONNECT\r\n"]),
    ));

    let mut adapter: AdapterType = Adapter::new(client, channel.subscriber().unwrap(), timer);

    let mut socket = adapter.socket().unwrap();
    let error = adapter
        .connect(&mut socket, SocketAddr::from_str("127.0.0.1:5000").unwrap())
        .unwrap_err();

    assert_eq!(nb::Error::Other(Error::ConnectError(AtError::Parse)), error);
}

#[test]
fn test_connect_receiving_mode_cmd_sent_once() {
    let timer = MockTimer::new();
    let channel: PubSubChannel<CriticalSectionRawMutex, URCMessages<16>, 16, 1, 1> = PubSubChannel::new();
    let mut client = MockAtatClient::new(&channel);

    client.add_response(MockedCommand::ok(Some(b"AT+CIPMUX=1\r\n"), None));
    client.add_response(MockedCommand::ok(Some(b"AT+CIPRECVMODE=1\r\n"), None));
    client.add_response(MockedCommand::ok(
        Some(b"AT+CIPSTART=0,\"TCP\",\"127.0.0.1\",5000\r\n"),
        Some(&[b"0,CONNECT\r\n"]),
    ));

    let mut adapter: AdapterType = Adapter::new(client, channel.subscriber().unwrap(), timer);

    let mut socket1 = adapter.socket().unwrap();
    let mut socket2 = adapter.socket().unwrap();

    adapter
        .connect(&mut socket1, SocketAddr::from_str("127.0.0.1:5000").unwrap())
        .unwrap();

    adapter.client.add_response(MockedCommand::ok(
        Some(b"AT+CIPSTART=1,\"TCP\",\"127.0.0.1\",6000\r\n"),
        Some(&[b"1,CONNECT\r\n"]),
    ));

    adapter
        .connect(&mut socket2, SocketAddr::from_str("127.0.0.1:6000").unwrap())
        .unwrap();
    adapter.client.assert_all_cmds_sent();
}

#[test]
fn test_connect_closing_socket_reconnected() {
    let timer = MockTimer::new();
    let channel: PubSubChannel<CriticalSectionRawMutex, URCMessages<16>, 16, 1, 1> = PubSubChannel::new();
    let mut client = MockAtatClient::new(&channel);

    client.add_response(MockedCommand::ok(Some(b"AT+CIPMUX=1\r\n"), None));
    client.add_response(MockedCommand::ok(Some(b"AT+CIPRECVMODE=1\r\n"), None));

    let mut adapter: AdapterType = Adapter::new(client, channel.subscriber().unwrap(), timer);

    let mut socket = adapter.socket().unwrap();

    adapter.client.add_urc_first_socket_closed();
    adapter.client.add_response(MockedCommand::ok(
        Some(b"AT+CIPSTART=0,\"TCP\",\"127.0.0.1\",5000\r\n"),
        Some(&[b"0,CONNECT\r\n"]),
    ));

    adapter
        .connect(&mut socket, SocketAddr::from_str("127.0.0.1:5000").unwrap())
        .unwrap();
    adapter.client.assert_all_cmds_sent();
}

#[test]
fn test_connect_already_connected_at_second_call() {
    let timer = MockTimer::new();
    let channel: PubSubChannel<CriticalSectionRawMutex, URCMessages<16>, 16, 1, 1> = PubSubChannel::new();
    let mut client = MockAtatClient::new(&channel);

    client.add_response(MockedCommand::ok(Some(b"AT+CIPMUX=1\r\n"), None));
    client.add_response(MockedCommand::ok(Some(b"AT+CIPRECVMODE=1\r\n"), None));
    client.add_response(MockedCommand::ok(
        Some(b"AT+CIPSTART=0,\"TCP\",\"127.0.0.1\",5000\r\n"),
        Some(&[b"0,CONNECT\r\n"]),
    ));

    client.add_urc_first_socket_connected();

    let mut adapter: AdapterType = Adapter::new(client, channel.subscriber().unwrap(), timer);

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
    let timer = MockTimer::new();
    let channel: PubSubChannel<CriticalSectionRawMutex, URCMessages<16>, 16, 1, 1> = PubSubChannel::new();
    let mut client = MockAtatClient::new(&channel);

    client.add_response(MockedCommand::ok(Some(b"AT+CIPMUX=1\r\n"), None));
    client.add_response(MockedCommand::ok(Some(b"AT+CIPRECVMODE=1\r\n"), None));
    client.add_response(MockedCommand::ok(
        Some(b"AT+CIPSTART=0,\"TCP\",\"127.0.0.1\",6000\r\n"),
        None,
    ));

    let mut adapter: AdapterType = Adapter::new(client, channel.subscriber().unwrap(), timer);

    let mut socket = adapter.socket().unwrap();
    let error = adapter
        .connect(&mut socket, SocketAddr::from_str("127.0.0.1:6000").unwrap())
        .unwrap_err();

    assert_eq!(nb::Error::Other(Error::UnconfirmedSocketState), error);
}

#[test]
fn test_connect_available_data_reset() {
    let timer = MockTimer::new();
    let channel: PubSubChannel<CriticalSectionRawMutex, URCMessages<16>, 16, 1, 1> = PubSubChannel::new();
    let mut client = MockAtatClient::new(&channel);

    client.add_response(MockedCommand::ok(Some(b"AT+CIPMUX=1\r\n"), None));
    client.add_response(MockedCommand::ok(Some(b"AT+CIPRECVMODE=1\r\n"), None));
    client.add_response(MockedCommand::ok(
        Some(b"AT+CIPSTART=0,\"TCP\",\"127.0.0.1\",5000\r\n"),
        Some(&[b"0,CONNECT\r\n"]),
    ));

    client.add_urc_first_socket_connected();

    let mut adapter: AdapterType = Adapter::new(client, channel.subscriber().unwrap(), timer);

    let mut socket = adapter.socket().unwrap();
    assert_eq!(0, socket.link_id);

    adapter
        .connect(&mut socket, SocketAddr::from_str("127.0.0.1:5000").unwrap())
        .unwrap();

    // Eight bytes of data available to receive()
    adapter.client.add_urc_message(b"+IPD,0,8\r\n");
    adapter.process_urc_messages();

    adapter.client.add_urc_first_socket_closed();
    adapter.close(socket).unwrap();

    let mut socket = adapter.socket().unwrap();
    assert_eq!(0, socket.link_id);

    // Connect command
    adapter.client.add_response(MockedCommand::ok(
        Some(b"AT+CIPSTART=0,\"TCP\",\"127.0.0.1\",5000\r\n"),
        Some(&[b"0,CONNECT\r\n"]),
    ));

    adapter
        .connect(&mut socket, SocketAddr::from_str("127.0.0.1:5000").unwrap())
        .unwrap();

    // No data available
    let error = adapter.receive(&mut socket, &mut [0x0; 32]).unwrap_err();
    assert_eq!(nb::Error::WouldBlock, error);
}

#[test]
fn test_send_not_connected() {
    let timer = MockTimer::new();
    let channel: PubSubChannel<CriticalSectionRawMutex, URCMessages<16>, 16, 1, 1> = PubSubChannel::new();
    let mut client = MockAtatClient::new(&channel);

    // Multiple connections command
    client.add_response(MockedCommand::ok(Some(b"AT+CIPMUX=1\r\n"), None));

    let mut adapter: AdapterType = Adapter::new(client, channel.subscriber().unwrap(), timer);
    let mut socket = adapter.socket().unwrap();

    let error = adapter.send(&mut socket, b"test data").unwrap_err();
    assert_eq!(nb::Error::Other(Error::SocketUnconnected), error);
}

#[test]
fn test_send_tx_prepare_error() {
    let timer = MockTimer::new();
    let channel: PubSubChannel<CriticalSectionRawMutex, URCMessages<16>, 16, 1, 1> = PubSubChannel::new();
    let client = MockAtatClient::new(&channel);
    let mut adapter: AdapterType = Adapter::new(client, channel.subscriber().unwrap(), timer);
    let mut socket = connect_socket(&mut adapter);

    adapter
        .client
        .add_response(MockedCommand::error(Some(b"AT+CIPSEND=0,9\r\n"), None));

    let error = adapter.send(&mut socket, b"test data").unwrap_err();
    assert_eq!(nb::Error::Other(Error::TransmissionStartFailed(AtError::Parse)), error);
}

#[test]
fn test_send_timer_start_error() {
    let mut timer = MockTimer::new();
    timer.expect_start().times(1).returning(|duration| {
        assert_eq!(duration, MockTimer::duration_ms(5_000));
        Err(100)
    });

    let channel: PubSubChannel<CriticalSectionRawMutex, URCMessages<16>, 16, 1, 1> = PubSubChannel::new();
    let client = MockAtatClient::new(&channel);
    let mut adapter: AdapterType = Adapter::new(client, channel.subscriber().unwrap(), timer);
    let mut socket = connect_socket(&mut adapter);

    adapter
        .client
        .add_response(MockedCommand::ok(Some(b"AT+CIPSEND=0,9\r\n"), None));
    adapter.client.add_response(MockedCommand::ok(Some(b"test data"), None));

    let error = adapter.send(&mut socket, b"test data").unwrap_err();
    assert_eq!(nb::Error::Other(Error::TimerError), error);
}

#[test]
fn test_send_timer_wait_error() {
    let mut timer = MockTimer::new();
    timer.expect_start().times(1).returning(|duration| {
        assert_eq!(duration, MockTimer::duration_ms(5_000));
        Ok(())
    });

    timer
        .expect_wait()
        .times(1)
        .returning(|| nb::Result::Err(nb::Error::Other(100)));

    let channel: PubSubChannel<CriticalSectionRawMutex, URCMessages<16>, 16, 1, 1> = PubSubChannel::new();
    let client = MockAtatClient::new(&channel);
    let mut adapter: AdapterType = Adapter::new(client, channel.subscriber().unwrap(), timer);
    let mut socket = connect_socket(&mut adapter);

    adapter
        .client
        .add_response(MockedCommand::ok(Some(b"AT+CIPSEND=0,9\r\n"), None));
    adapter.client.add_response(MockedCommand::ok(Some(b"test data"), None));

    let error = adapter.send(&mut socket, b"test data").unwrap_err();
    assert_eq!(nb::Error::Other(Error::TimerError), error);
}

#[test]
fn test_send_timeout() {
    let mut timer = MockTimer::new();
    timer.expect_start().times(1).returning(|duration| {
        assert_eq!(duration, MockTimer::duration_ms(5_000));
        Ok(())
    });

    timer.expect_wait().times(1).returning(|| nb::Result::Ok(()));

    let channel: PubSubChannel<CriticalSectionRawMutex, URCMessages<16>, 16, 1, 1> = PubSubChannel::new();
    let client = MockAtatClient::new(&channel);
    let mut adapter: AdapterType = Adapter::new(client, channel.subscriber().unwrap(), timer);
    let mut socket = connect_socket(&mut adapter);

    adapter
        .client
        .add_response(MockedCommand::ok(Some(b"AT+CIPSEND=0,9\r\n"), None));
    adapter.client.add_response(MockedCommand::ok(Some(b"test data"), None));

    let error = adapter.send(&mut socket, b"test data").unwrap_err();
    assert_eq!(nb::Error::Other(Error::SendFailed(AtError::Timeout)), error);
}
//
#[test]
fn test_send_byte_count_not_matching() {
    let mut timer = MockTimer::new();
    timer.expect_start().times(1).returning(|_| Ok(()));

    let channel: PubSubChannel<CriticalSectionRawMutex, URCMessages<16>, 16, 1, 1> = PubSubChannel::new();
    let client = MockAtatClient::new(&channel);
    let mut adapter: AdapterType = Adapter::new(client, channel.subscriber().unwrap(), timer);
    let mut socket = connect_socket(&mut adapter);

    adapter.client.add_response(MockedCommand::ok(
        Some(b"AT+CIPSEND=0,9\r\n"),
        Some(&[b"Recv 4 bytes\r\n"]),
    ));
    adapter
        .client
        .add_response(MockedCommand::ok(Some(b"test data"), Some(&[b"SEND OK\r\n"])));

    let error = adapter.send(&mut socket, b"test data").unwrap_err();
    assert_eq!(nb::Error::Other(Error::PartialSend), error);
}

#[test]
fn test_send_ok_without_recv_message() {
    let mut timer = MockTimer::new();
    timer.expect_start().times(1).returning(|_| Ok(()));

    let channel: PubSubChannel<CriticalSectionRawMutex, URCMessages<16>, 16, 1, 1> = PubSubChannel::new();
    let client = MockAtatClient::new(&channel);
    let mut adapter: AdapterType = Adapter::new(client, channel.subscriber().unwrap(), timer);
    let mut socket = connect_socket(&mut adapter);

    adapter
        .client
        .add_response(MockedCommand::ok(Some(b"AT+CIPSEND=0,9\r\n"), None));
    adapter
        .client
        .add_response(MockedCommand::ok(Some(b"test data"), Some(&[b"SEND OK\r\n"])));

    adapter.send(&mut socket, b"test data").unwrap();
}

#[test]
fn test_send_fail_urc_message() {
    let mut timer = MockTimer::new();
    timer.expect_start().times(1).returning(|_| Ok(()));

    let channel: PubSubChannel<CriticalSectionRawMutex, URCMessages<16>, 16, 1, 1> = PubSubChannel::new();
    let client = MockAtatClient::new(&channel);
    let mut adapter: AdapterType = Adapter::new(client, channel.subscriber().unwrap(), timer);
    let mut socket = connect_socket(&mut adapter);

    adapter.client.add_response(MockedCommand::ok(
        Some(b"AT+CIPSEND=0,4\r\n"),
        Some(&[b"Recv 4 bytes\r\n"]),
    ));
    adapter
        .client
        .add_response(MockedCommand::ok(Some(b"test"), Some(&[b"SEND FAIL\r\n"])));

    let error = adapter.send(&mut socket, b"test").unwrap_err();
    assert_eq!(nb::Error::Other(Error::SendFailed(AtError::Error)), error);
}

#[test]
fn test_send_error_and_recv_bytes_not_matching() {
    let mut timer = MockTimer::new();
    timer.expect_start().times(1).returning(|_| Ok(()));

    let channel: PubSubChannel<CriticalSectionRawMutex, URCMessages<16>, 16, 1, 1> = PubSubChannel::new();
    let client = MockAtatClient::new(&channel);
    let mut adapter: AdapterType = Adapter::new(client, channel.subscriber().unwrap(), timer);
    let mut socket = connect_socket(&mut adapter);

    adapter.client.add_response(MockedCommand::ok(
        Some(b"AT+CIPSEND=0,9\r\n"),
        Some(&[b"Recv 4 bytes\r\n"]),
    ));
    adapter
        .client
        .add_response(MockedCommand::ok(Some(b"test data"), Some(&[b"SEND FAIL\r\n"])));

    let error = adapter.send(&mut socket, b"test data").unwrap_err();
    assert_eq!(nb::Error::Other(Error::SendFailed(AtError::Error)), error);
}

#[test]
fn test_send_correct_commands() {
    let mut timer = MockTimer::new();
    timer.expect_start().times(1).returning(|_| Ok(()));

    let channel: PubSubChannel<CriticalSectionRawMutex, URCMessages<16>, 16, 1, 1> = PubSubChannel::new();
    let client = MockAtatClient::new(&channel);
    let mut adapter: AdapterType = Adapter::new(client, channel.subscriber().unwrap(), timer);
    let mut socket = connect_socket(&mut adapter);

    adapter.client.add_response(MockedCommand::ok(
        Some(b"AT+CIPSEND=0,4\r\n"),
        Some(&[b"Recv 4 bytes\r\n"]),
    ));
    adapter
        .client
        .add_response(MockedCommand::ok(Some(b"test"), Some(&[b"SEND OK\r\n"])));

    let sent_bytes = adapter.send(&mut socket, b"test").unwrap();
    assert_eq!(4, sent_bytes);
    adapter.client.assert_all_cmds_sent();
}

#[test]
fn test_send_multiple_calls_urc_status_reset() {
    let mut timer = MockTimer::new();
    timer.expect_start().times(2).returning(|_| Ok(()));

    let channel: PubSubChannel<CriticalSectionRawMutex, URCMessages<16>, 16, 1, 1> = PubSubChannel::new();
    let client = MockAtatClient::new(&channel);
    let mut adapter: AdapterType = Adapter::new(client, channel.subscriber().unwrap(), timer);
    let mut socket = connect_socket(&mut adapter);

    adapter.client.add_response(MockedCommand::ok(
        Some(b"AT+CIPSEND=0,4\r\n"),
        Some(&[b"Recv 4 bytes\r\n"]),
    ));
    adapter
        .client
        .add_response(MockedCommand::ok(Some(b"test"), Some(&[b"SEND FAIL\r\n"])));

    assert!(adapter.send(&mut socket, b"test").is_err());

    adapter.client.add_response(MockedCommand::ok(
        Some(b"AT+CIPSEND=0,9\r\n"),
        Some(&[b"Recv 9 bytes\r\n"]),
    ));
    adapter
        .client
        .add_response(MockedCommand::ok(Some(b"test data"), Some(&[b"SEND OK\r\n"])));

    let sent_bytes = adapter.send(&mut socket, b"test data").unwrap();
    assert_eq!(9, sent_bytes);
    adapter.client.assert_all_cmds_sent();
}

#[test]
fn test_send_chunks() {
    let mut timer = MockTimer::new();
    timer.expect_start().times(3).returning(|_| Ok(()));

    let channel: PubSubChannel<CriticalSectionRawMutex, URCMessages<16>, 16, 1, 1> = PubSubChannel::new();
    let client = MockAtatClient::new(&channel);
    let mut adapter: AdapterType = Adapter::new(client, channel.subscriber().unwrap(), timer);
    let mut socket = connect_socket(&mut adapter);

    adapter.client.add_response(MockedCommand::ok(
        Some(b"AT+CIPSEND=0,32\r\n"),
        Some(&[b"Recv 32 bytes\r\n"]),
    ));
    adapter
        .client
        .add_response(MockedCommand::ok(Some(&[b'A'; 32]), Some(&[b"SEND OK\r\n"])));

    adapter.client.add_response(MockedCommand::ok(
        Some(b"AT+CIPSEND=0,32\r\n"),
        Some(&[b"Recv 32 bytes\r\n"]),
    ));
    adapter
        .client
        .add_response(MockedCommand::ok(Some(&[b'A'; 32]), Some(&[b"SEND OK\r\n"])));

    adapter.client.add_response(MockedCommand::ok(
        Some(b"AT+CIPSEND=0,14\r\n"),
        Some(&[b"Recv 14 bytes\r\n"]),
    ));
    adapter
        .client
        .add_response(MockedCommand::ok(Some(b"second message"), Some(&[b"SEND OK\r\n"])));

    let mut buffer = vec![b'A'; 64];
    buffer.extend_from_slice(b"second message");

    let sent_bytes = adapter.send(&mut socket, buffer.as_slice()).unwrap();
    assert_eq!(78, sent_bytes);
    adapter.client.assert_all_cmds_sent();
}

#[test]
fn test_receive_no_data_available() {
    let timer = MockTimer::new();
    let channel: PubSubChannel<CriticalSectionRawMutex, URCMessages<16>, 16, 1, 1> = PubSubChannel::new();
    let client = MockAtatClient::new(&channel);
    let mut adapter: AdapterType = Adapter::new(client, channel.subscriber().unwrap(), timer);
    let mut socket = connect_socket(&mut adapter);

    // Other socket
    adapter.client.add_urc_message(b"+IPD,3,256\r\n");

    let mut buffer = [0x0; 32];
    let error = adapter.receive(&mut socket, &mut buffer).unwrap_err();

    assert_eq!([0x0; 32], buffer);
    assert_eq!(nb::Error::WouldBlock, error);
}

#[test]
fn test_receive_after_restart() {
    let mut timer = MockTimer::new();
    timer.expect_start().times(1).returning(|_| Ok(()));
    timer
        .expect_wait()
        .times(1)
        .returning(|| nb::Result::Err(nb::Error::WouldBlock));

    let channel: PubSubChannel<CriticalSectionRawMutex, URCMessages<16>, 16, 1, 1> = PubSubChannel::new();
    let client = MockAtatClient::new(&channel);
    let mut adapter: AdapterType = Adapter::new(client, channel.subscriber().unwrap(), timer);
    let mut socket = connect_socket(&mut adapter);

    // Fake available data
    adapter.client.add_urc_message(b"+IPD,0,256\r\n");
    adapter.process_urc_messages();

    // Restart command
    adapter
        .client
        .add_response(MockedCommand::ok(Some(b"AT+RST\r\n"), Some(&[b"ready\r\n"])));

    adapter.restart().unwrap();

    let mut buffer = [0x0; 32];
    let error = adapter.receive(&mut socket, &mut buffer).unwrap_err();

    // Assert that available data is reset on restart
    assert_eq!([0x0; 32], buffer);
    assert_eq!(nb::Error::WouldBlock, error);
    adapter.client.assert_all_cmds_sent();
}

#[test]
fn test_receive_receive_command_failed() {
    let timer = MockTimer::new();
    let channel: PubSubChannel<CriticalSectionRawMutex, URCMessages<16>, 16, 1, 1> = PubSubChannel::new();
    let client = MockAtatClient::new(&channel);
    let mut adapter: AdapterType = Adapter::new(client, channel.subscriber().unwrap(), timer);
    let mut socket = connect_socket(&mut adapter);

    adapter.client.add_urc_message(b"+IPD,0,256\r\n");
    adapter
        .client
        .add_response(MockedCommand::error(Some(b"AT+CIPRECVDATA=0,16\r\n"), None));

    let mut buffer = [0x0; 32];
    let error = adapter.receive(&mut socket, &mut buffer).unwrap_err();
    assert_eq!(nb::Error::Other(Error::ReceiveFailed(AtError::Parse)), error);
}

#[test]
fn test_receive_no_data_received() {
    let timer = MockTimer::new();
    let channel: PubSubChannel<CriticalSectionRawMutex, URCMessages<16>, 16, 1, 1> = PubSubChannel::new();
    let client = MockAtatClient::new(&channel);
    let mut adapter: AdapterType = Adapter::new(client, channel.subscriber().unwrap(), timer);
    let mut socket = connect_socket(&mut adapter);

    adapter.client.add_urc_message(b"+IPD,0,256\r\n");
    adapter
        .client
        .add_response(MockedCommand::ok(Some(b"AT+CIPRECVDATA=0,16\r\n"), None));

    let mut buffer = [0x0; 32];
    let error = adapter.receive(&mut socket, &mut buffer).unwrap_err();
    assert_eq!(nb::Error::Other(Error::ReceiveFailed(AtError::InvalidResponse)), error);
}

#[test]
fn test_receive_correct_command() {
    let timer = MockTimer::new();
    let channel: PubSubChannel<CriticalSectionRawMutex, URCMessages<16>, 16, 1, 1> = PubSubChannel::new();
    let client = MockAtatClient::new(&channel);
    let mut adapter: AdapterType = Adapter::new(client, channel.subscriber().unwrap(), timer);
    let mut socket = connect_socket(&mut adapter);

    adapter.client.add_urc_message(b"+IPD,0,4\r\n");
    adapter.client.add_response(MockedCommand::ok(
        Some(b"AT+CIPRECVDATA=0,16\r\n"),
        Some(&[b"+CIPRECVDATA:4,aaaa"]),
    ));

    let mut buffer = [b' '; 16];
    let length = adapter.receive(&mut socket, &mut buffer).unwrap();

    assert_eq!(4, length);
    assert_eq!(b"aaaa", &buffer[..4]);
    adapter.client.assert_all_cmds_sent();
}

#[test]
/// Out-of-spec response covering bug in older ESP-AT firmware versions
/// See https://github.com/atlas-aero/rt-esp-at-nal/issues/23
fn test_receive_correct_command_out_of_spec() {
    let timer = MockTimer::new();
    let channel: PubSubChannel<CriticalSectionRawMutex, URCMessages<16>, 16, 1, 1> = PubSubChannel::new();
    let client = MockAtatClient::new(&channel);
    let mut adapter: AdapterType = Adapter::new(client, channel.subscriber().unwrap(), timer);
    let mut socket = connect_socket(&mut adapter);

    adapter.client.add_urc_message(b"+IPD,0,4\r\n");
    adapter.client.add_response(MockedCommand::ok(
        Some(b"AT+CIPRECVDATA=0,16\r\n"),
        Some(&[b"+CIPRECVDATA,4:aaaa"]),
    ));

    let mut buffer = [b' '; 16];
    let length = adapter.receive(&mut socket, &mut buffer).unwrap();

    assert_eq!(4, length);
    assert_eq!(b"aaaa", &buffer[..4]);
    adapter.client.assert_all_cmds_sent();
}

#[test]
fn test_receive_data_received_buffer_bigger_then_block_size() {
    let timer = MockTimer::new();
    let channel: PubSubChannel<CriticalSectionRawMutex, URCMessages<16>, 16, 1, 1> = PubSubChannel::new();
    let client = MockAtatClient::new(&channel);
    let mut adapter: AdapterType = Adapter::new(client, channel.subscriber().unwrap(), timer);
    let mut socket = connect_socket(&mut adapter);

    adapter.client.add_urc_message(b"+IPD,0,24\r\n");
    adapter.client.add_response(MockedCommand::ok(
        Some(b"AT+CIPRECVDATA=0,16\r\n"),
        Some(&[b"+CIPRECVDATA:16,aaaaaaaaaaaaaaaa"]),
    ));
    adapter.client.add_response(MockedCommand::ok(
        Some(b"AT+CIPRECVDATA=0,16\r\n"),
        Some(&[b"+CIPRECVDATA:8,bbbbbbbb"]),
    ));

    let mut buffer = [b' '; 64];
    let length = adapter.receive(&mut socket, &mut buffer).unwrap();

    assert_eq!(24, length);
    assert_eq!(b"aaaaaaaaaaaaaaaabbbbbbbb", &buffer[..length]);
    adapter.client.assert_all_cmds_sent();
}

#[test]
fn test_receive_data_received_buffer_smaller_then_block_size() {
    let timer = MockTimer::new();
    let channel: PubSubChannel<CriticalSectionRawMutex, URCMessages<16>, 16, 1, 1> = PubSubChannel::new();
    let client = MockAtatClient::new(&channel);
    let mut adapter: AdapterType = Adapter::new(client, channel.subscriber().unwrap(), timer);
    let mut socket = connect_socket(&mut adapter);

    // Signal data is available
    adapter.client.add_urc_message(b"+IPD,0,5\r\n");

    adapter.client.add_response(MockedCommand::ok(
        Some(b"AT+CIPRECVDATA=0,2\r\n"),
        Some(&[b"+CIPRECVDATA:2,aa"]),
    ));
    let mut buffer = [b' '; 2];
    let length = adapter.receive(&mut socket, &mut buffer).unwrap();
    assert_eq!(2, length);
    assert_eq!(b"aa", &buffer);

    adapter.client.add_response(MockedCommand::ok(
        Some(b"AT+CIPRECVDATA=0,2\r\n"),
        Some(&[b"+CIPRECVDATA:2,bb"]),
    ));
    let mut buffer = [b' '; 2];
    let length = adapter.receive(&mut socket, &mut buffer).unwrap();
    assert_eq!(2, length);
    assert_eq!(b"bb", &buffer);

    adapter.client.add_response(MockedCommand::ok(
        Some(b"AT+CIPRECVDATA=0,2\r\n"),
        Some(&[b"+CIPRECVDATA:1,c"]),
    ));
    let mut buffer = [b' '; 2];
    let length = adapter.receive(&mut socket, &mut buffer).unwrap();
    assert_eq!(1, length);
    assert_eq!(b"c ", &buffer);
    adapter.client.assert_all_cmds_sent();
}

#[test]
fn test_receive_data_received_less_data_received_then_requested() {
    let timer = MockTimer::new();
    let channel: PubSubChannel<CriticalSectionRawMutex, URCMessages<16>, 16, 1, 1> = PubSubChannel::new();
    let client = MockAtatClient::new(&channel);
    let mut adapter: AdapterType = Adapter::new(client, channel.subscriber().unwrap(), timer);
    let mut socket = connect_socket(&mut adapter);

    adapter.client.add_urc_message(b"+IPD,0,10\r\n");
    adapter.client.add_response(MockedCommand::ok(
        Some(b"AT+CIPRECVDATA=0,16\r\n"),
        Some(&[b"+CIPRECVDATA:4,aaaa"]),
    ));
    adapter.client.add_response(MockedCommand::ok(
        Some(b"AT+CIPRECVDATA=0,16\r\n"),
        Some(&[b"+CIPRECVDATA:4,bbbb"]),
    ));
    adapter.client.add_response(MockedCommand::ok(
        Some(b"AT+CIPRECVDATA=0,16\r\n"),
        Some(&[b"+CIPRECVDATA:2,cc"]),
    ));

    let mut buffer = [b' '; 32];
    let length = adapter.receive(&mut socket, &mut buffer).unwrap();

    assert_eq!(10, length);
    assert_eq!(b"aaaabbbbcc", &buffer[..length]);
    adapter.client.assert_all_cmds_sent();
}

/// This can just happen if ESP-AT sends more data then requested, which is a protocol violation.
#[test]
fn test_receive_data_received_more_data_received_then_block_size() {
    let timer = MockTimer::new();
    let channel: PubSubChannel<CriticalSectionRawMutex, URCMessages<16>, 16, 1, 1> = PubSubChannel::new();
    let client = MockAtatClient::new(&channel);
    let mut adapter: AdapterType = Adapter::new(client, channel.subscriber().unwrap(), timer);
    let mut socket = connect_socket(&mut adapter);

    adapter.client.add_urc_message(b"+IPD,0,20\r\n");
    adapter.client.add_response(MockedCommand::ok(
        Some(b"AT+CIPRECVDATA=0,16\r\n"),
        Some(&[b"+CIPRECVDATA:16,aaaaaaaaaaaaaaaaa"]),
    ));

    let mut buffer = [b' '; 16];
    let error = adapter.receive(&mut socket, &mut buffer).unwrap_err();
    assert_eq!(nb::Error::Other(Error::ReceiveFailed(AtError::InvalidResponse)), error);
}

/// This can just happen if ESP-AT sends more data then requested, which is a protocol violation.
#[test]
fn test_receive_data_received_buffer_overflow() {
    let timer = MockTimer::new();
    let channel: PubSubChannel<CriticalSectionRawMutex, URCMessages<16>, 16, 1, 1> = PubSubChannel::new();
    let client = MockAtatClient::new(&channel);
    let mut adapter: AdapterType = Adapter::new(client, channel.subscriber().unwrap(), timer);
    let mut socket = connect_socket(&mut adapter);

    adapter.client.add_urc_message(b"+IPD,0,20\r\n");
    adapter.client.add_response(MockedCommand::ok(
        Some(b"AT+CIPRECVDATA=0,16\r\n"),
        Some(&[b"+CIPRECVDATA:16,aaaaaaaaaaaaaaaa"]),
    ));
    adapter.client.add_response(MockedCommand::ok(
        Some(b"AT+CIPRECVDATA=0,4\r\n"),
        Some(&[b"+CIPRECVDATA:5,aaaaa"]),
    ));

    let mut buffer = [b' '; 20];
    let error = adapter.receive(&mut socket, &mut buffer).unwrap_err();
    assert_eq!(nb::Error::Other(Error::ReceiveOverflow), error);
}

#[test]
fn test_close_socket_not_connected_yet() {
    let timer = MockTimer::new();
    let channel: PubSubChannel<CriticalSectionRawMutex, URCMessages<16>, 16, 1, 1> = PubSubChannel::new();
    let client = MockAtatClient::new(&channel);
    let mut adapter: AdapterType = Adapter::new(client, channel.subscriber().unwrap(), timer);

    // Receiving socket
    adapter.client.add_response(MockedCommand::ok(Some(b"AT+CIPMUX=1\r\n"), None));
    let socket = adapter.socket().unwrap();
    assert_eq!(0, socket.link_id);

    adapter.close(socket).unwrap();

    // Socket is available for reuse
    let socket = adapter.socket().unwrap();
    assert_eq!(0, socket.link_id);
    adapter.client.assert_all_cmds_sent();
}

#[test]
fn test_close_socket_already_closed_by_remote() {
    let timer = MockTimer::new();
    let channel: PubSubChannel<CriticalSectionRawMutex, URCMessages<16>, 16, 1, 1> = PubSubChannel::new();
    let client = MockAtatClient::new(&channel);
    let mut adapter: AdapterType = Adapter::new(client, channel.subscriber().unwrap(), timer);
    let socket = connect_socket(&mut adapter);

    adapter.client.add_urc_first_socket_closed();
    adapter.close(socket).unwrap();

    // Socket is available for reuse
    let socket = adapter.socket().unwrap();
    assert_eq!(0, socket.link_id);

    adapter.client.assert_all_cmds_sent();
}

#[test]
fn test_close_open_socket() {
    let timer = MockTimer::new();
    let channel: PubSubChannel<CriticalSectionRawMutex, URCMessages<16>, 16, 1, 1> = PubSubChannel::new();
    let client = MockAtatClient::new(&channel);
    let mut adapter: AdapterType = Adapter::new(client, channel.subscriber().unwrap(), timer);

    // Receiving socket
    adapter.client.add_response(MockedCommand::ok(Some(b"AT+CIPMUX=1\r\n"), None));
    let socket = adapter.socket().unwrap();

    adapter.close(socket).unwrap();

    // Socket is available for reuse
    let socket = adapter.socket().unwrap();
    assert_eq!(0, socket.link_id);

    adapter.client.assert_all_cmds_sent();
}

#[test]
fn test_close_after_restart() {
    let mut timer = MockTimer::new();
    timer.expect_start().times(1).returning(|_| Ok(()));
    timer
        .expect_wait()
        .times(1)
        .returning(|| nb::Result::Err(nb::Error::WouldBlock));

    let channel: PubSubChannel<CriticalSectionRawMutex, URCMessages<16>, 16, 1, 1> = PubSubChannel::new();
    let client = MockAtatClient::new(&channel);
    let mut adapter: AdapterType = Adapter::new(client, channel.subscriber().unwrap(), timer);
    let socket = connect_socket(&mut adapter);

    // Response to RST command
    adapter
        .client
        .add_response(MockedCommand::ok(Some(b"AT+RST\r\n"), Some(&[b"ready\r\n"])));
    adapter.restart().unwrap();

    adapter.close(socket).unwrap();
    adapter.client.assert_all_cmds_sent();

    // Socket is available for reuse
    adapter.client.add_response(MockedCommand::ok(Some(b"AT+CIPMUX=1\r\n"), None));
    let socket = adapter.socket().unwrap();
    assert_eq!(0, socket.link_id);
}

#[test]
fn test_close_socket_command_error() {
    let timer = MockTimer::new();
    let channel: PubSubChannel<CriticalSectionRawMutex, URCMessages<16>, 16, 1, 1> = PubSubChannel::new();
    let client = MockAtatClient::new(&channel);
    let mut adapter: AdapterType = Adapter::new(client, channel.subscriber().unwrap(), timer);
    let socket = connect_socket(&mut adapter);

    adapter
        .client
        .add_response(MockedCommand::error(Some(b"AT+CIPCLOSE=0\r\n"), None));
    let error = adapter.close(socket).unwrap_err();
    assert_eq!(Error::CloseError(AtError::Parse), error);

    // Socket is available for reuse
    adapter.client.add_urc_first_socket_connected();
    let socket = adapter.socket().unwrap();
    assert_eq!(0, socket.link_id);
    adapter.client.assert_all_cmds_sent();
}

#[test]
fn test_close_socket_unconfirmed() {
    let timer = MockTimer::new();
    let channel: PubSubChannel<CriticalSectionRawMutex, URCMessages<16>, 16, 1, 1> = PubSubChannel::new();
    let client = MockAtatClient::new(&channel);
    let mut adapter: AdapterType = Adapter::new(client, channel.subscriber().unwrap(), timer);
    let socket = connect_socket(&mut adapter);

    adapter.client.add_response(MockedCommand::ok(Some(b"AT+CIPCLOSE=0\r\n"), None));
    let error = adapter.close(socket).unwrap_err();
    assert_eq!(Error::UnconfirmedSocketState, error);

    // Socket is available for reuse
    let socket = adapter.socket().unwrap();
    assert_eq!(0, socket.link_id);
    adapter.client.assert_all_cmds_sent();
}

#[test]
fn test_close_socket_closed_successfully() {
    let timer = MockTimer::new();
    let channel: PubSubChannel<CriticalSectionRawMutex, URCMessages<16>, 16, 1, 1> = PubSubChannel::new();
    let client = MockAtatClient::new(&channel);
    let mut adapter: AdapterType = Adapter::new(client, channel.subscriber().unwrap(), timer);
    let socket = connect_socket(&mut adapter);

    // Dummy URC for first URC check call
    adapter.client.add_urc_wifi_got_ip();

    adapter
        .client
        .add_response(MockedCommand::ok(Some(b"AT+CIPCLOSE=0\r\n"), Some(&[b"0,CLOSED\r\n"])));
    adapter.close(socket).unwrap();

    // Socket is available for reuse
    let socket = adapter.socket().unwrap();
    assert_eq!(0, socket.link_id);
    adapter.client.assert_all_cmds_sent();
}

#[test]
fn test_is_connected_open() {
    let timer = MockTimer::new();
    let channel: PubSubChannel<CriticalSectionRawMutex, URCMessages<16>, 16, 1, 1> = PubSubChannel::new();
    let client = MockAtatClient::new(&channel);
    let mut adapter: AdapterType = Adapter::new(client, channel.subscriber().unwrap(), timer);

    // Receiving socket
    adapter.client.add_response(MockedCommand::ok(Some(b"AT+CIPMUX=1\r\n"), None));
    let socket = adapter.socket().unwrap();

    assert!(!adapter.is_connected(&socket).unwrap());
}

#[test]
fn test_is_connected_true() {
    let timer = MockTimer::new();
    let channel: PubSubChannel<CriticalSectionRawMutex, URCMessages<16>, 16, 1, 1> = PubSubChannel::new();
    let client = MockAtatClient::new(&channel);
    let mut adapter: AdapterType = Adapter::new(client, channel.subscriber().unwrap(), timer);

    // Receiving socket
    adapter.client.add_response(MockedCommand::ok(Some(b"AT+CIPMUX=1\r\n"), None));
    let socket = adapter.socket().unwrap();

    adapter.client.add_urc_first_socket_connected();

    assert!(adapter.is_connected(&socket).unwrap());
}

#[test]
fn test_is_connected_closing() {
    let timer = MockTimer::new();
    let channel: PubSubChannel<CriticalSectionRawMutex, URCMessages<16>, 16, 1, 1> = PubSubChannel::new();
    let client = MockAtatClient::new(&channel);
    let mut adapter: AdapterType = Adapter::new(client, channel.subscriber().unwrap(), timer);

    // Receiving socket
    adapter.client.add_response(MockedCommand::ok(Some(b"AT+CIPMUX=1\r\n"), None));
    let socket = adapter.socket().unwrap();

    adapter.client.add_urc_first_socket_closed();

    assert!(!adapter.is_connected(&socket).unwrap());
}

/// Helper for opening & connecting a socket
fn connect_socket(adapter: &mut AdapterType) -> Socket {
    adapter.client.add_response(MockedCommand::ok(Some(b"AT+CIPMUX=1\r\n"), None));
    adapter
        .client
        .add_response(MockedCommand::ok(Some(b"AT+CIPRECVMODE=1\r\n"), None));
    adapter.client.add_response(MockedCommand::ok(
        Some(b"AT+CIPSTART=0,\"TCP\",\"127.0.0.1\",5000\r\n"),
        Some(&[b"0,CONNECT\r\n"]),
    ));

    let mut socket = adapter.socket().unwrap();

    adapter
        .connect(&mut socket, SocketAddr::from_str("127.0.0.1:5000").unwrap())
        .unwrap();

    socket
}
