use crate::adapter::Adapter;
use crate::stack::{Error, Socket};
use crate::tests::mock::{MockAtatClient, MockTimer};
use alloc::string::{String, ToString};
use alloc::vec;
use atat::Error as AtError;
use core::str::FromStr;
use embedded_nal::{SocketAddr, TcpClientStack};

type AdapterType = Adapter<MockAtatClient, MockTimer, 1_000_000, 256, 4>;

#[test]
fn test_socket_multi_conn_error() {
    let timer = MockTimer::new();
    let mut client = MockAtatClient::new();
    client.add_error_response();

    let mut adapter: AdapterType = Adapter::new(client, timer);
    let result = adapter.socket().unwrap_err();
    assert_eq!(Error::EnablingMultiConnectionsFailed(AtError::Parse), result);

    let commands = adapter.client.get_commands_as_strings();
    assert_eq!(1, commands.len());
    assert_eq!("AT+CIPMUX=1\r\n".to_string(), commands[0]);
}

#[test]
fn test_socket_multi_conn_would_block() {
    let timer = MockTimer::new();
    let mut client = MockAtatClient::new();
    client.send_would_block(0);

    let mut adapter: AdapterType = Adapter::new(client, timer);
    let result = adapter.socket().unwrap_err();

    assert_eq!(Error::UnexpectedWouldBlock, result);
}

#[test]
fn test_socket_multi_conn_enabled_once() {
    let timer = MockTimer::new();
    let mut client = MockAtatClient::new();
    client.add_ok_response();

    let mut adapter: AdapterType = Adapter::new(client, timer);
    adapter.socket().unwrap();
    adapter.socket().unwrap();

    let commands = adapter.client.get_commands_as_strings();
    assert_eq!(1, commands.len());
    assert_eq!("AT+CIPMUX=1\r\n".to_string(), commands[0]);
}

#[test]
fn test_socket_opened() {
    let timer = MockTimer::new();
    let mut client = MockAtatClient::new();
    client.add_ok_response();

    let mut adapter: AdapterType = Adapter::new(client, timer);
    assert_eq!(0, adapter.socket().unwrap().link_id);
    assert_eq!(1, adapter.socket().unwrap().link_id);
    assert_eq!(2, adapter.socket().unwrap().link_id);
    assert_eq!(3, adapter.socket().unwrap().link_id);
    assert_eq!(4, adapter.socket().unwrap().link_id);
}

#[test]
fn test_socket_not_available() {
    let timer = MockTimer::new();
    let mut client = MockAtatClient::new();
    client.add_ok_response();

    let mut adapter: AdapterType = Adapter::new(client, timer);
    for _ in 0..5 {
        adapter.socket().unwrap();
    }

    let result = adapter.socket().unwrap_err();
    assert_eq!(Error::NoSocketAvailable, result);
}

#[test]
fn test_connect_already_connected_by_urc() {
    let timer = MockTimer::new();
    let mut client = MockAtatClient::new();

    // Multiple connections command
    client.add_ok_response();

    client.add_urc_first_socket_connected();
    let mut adapter: AdapterType = Adapter::new(client, timer);

    let mut socket = adapter.socket().unwrap();
    let error = adapter
        .connect(&mut socket, SocketAddr::from_str("127.0.0.1:5000").unwrap())
        .unwrap_err();

    assert_eq!(nb::Error::Other(Error::AlreadyConnected), error);
}

#[test]
fn test_connect_correct_commands_ipv4() {
    let timer = MockTimer::new();
    let mut client = MockAtatClient::new();

    // Multiple connections command
    client.add_ok_response();
    // Receiving mode command
    client.add_ok_response();
    // Connect command
    client.add_ok_response();

    client.skip_urc(1);
    client.add_urc_first_socket_connected();

    let mut adapter: AdapterType = Adapter::new(client, timer);

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
    let timer = MockTimer::new();
    let mut client = MockAtatClient::new();

    // Multiple connections command
    client.add_ok_response();
    // Receiving mode command
    client.add_ok_response();
    // Connect command
    client.add_ok_response();

    client.skip_urc(1);
    client.add_urc_first_socket_connected();

    let mut adapter: AdapterType = Adapter::new(client, timer);

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
    let timer = MockTimer::new();
    let mut client = MockAtatClient::new();

    // Multiple connections command
    client.add_ok_response();
    // Receiving mode command
    client.add_error_response();

    let mut adapter: AdapterType = Adapter::new(client, timer);

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
    let timer = MockTimer::new();
    let mut client = MockAtatClient::new();

    // Multiple connections command
    client.add_ok_response();
    // Receiving mode command
    client.add_ok_response();
    // Connect mode command
    client.send_would_block(2);

    let mut adapter: AdapterType = Adapter::new(client, timer);

    let mut socket = adapter.socket().unwrap();
    let error = adapter
        .connect(&mut socket, SocketAddr::from_str("127.0.0.1:5000").unwrap())
        .unwrap_err();

    assert_eq!(nb::Error::Other(Error::UnexpectedWouldBlock), error);
}

#[test]
fn test_connect_connect_command_error() {
    let timer = MockTimer::new();
    let mut client = MockAtatClient::new();

    // Multiple connections command
    client.add_ok_response();
    // Receiving mode command
    client.add_ok_response();
    // Connect command
    client.add_error_response();

    let mut adapter: AdapterType = Adapter::new(client, timer);

    let mut socket = adapter.socket().unwrap();
    let error = adapter
        .connect(&mut socket, SocketAddr::from_str("127.0.0.1:5000").unwrap())
        .unwrap_err();

    assert_eq!(nb::Error::Other(Error::ConnectError(AtError::Parse)), error);
}

#[test]
fn test_connect_connect_command_would_block() {
    let timer = MockTimer::new();
    let mut client = MockAtatClient::new();

    // Multiple connections command
    client.add_ok_response();
    // Receiving mode command
    client.send_would_block(1);

    let mut adapter: AdapterType = Adapter::new(client, timer);

    let mut socket = adapter.socket().unwrap();
    let error = adapter
        .connect(&mut socket, SocketAddr::from_str("127.0.0.1:5000").unwrap())
        .unwrap_err();

    assert_eq!(nb::Error::Other(Error::UnexpectedWouldBlock), error);
}

#[test]
fn test_connect_receiving_mode_cmd_sent_once() {
    let timer = MockTimer::new();
    let mut client = MockAtatClient::new();

    // Multiple connections command
    client.add_ok_response();

    // Receiving mode command
    client.add_ok_response();

    // First connect command
    client.add_ok_response();
    client.skip_urc(1);
    client.add_urc_first_socket_connected();

    let mut adapter: AdapterType = Adapter::new(client, timer);

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
    let timer = MockTimer::new();
    let mut client = MockAtatClient::new();

    // Multiple connections command
    client.add_ok_response();
    // Receiving mode command
    client.add_ok_response();

    let mut adapter: AdapterType = Adapter::new(client, timer);

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
    let timer = MockTimer::new();
    let mut client = MockAtatClient::new();

    // Multiple connections command
    client.add_ok_response();
    // Receiving mode command
    client.add_ok_response();
    // Connect command
    client.add_ok_response();

    client.skip_urc(1);
    client.add_urc_first_socket_connected();

    let mut adapter: AdapterType = Adapter::new(client, timer);

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
    let mut client = MockAtatClient::new();

    // Multiple connections command
    client.add_ok_response();
    // Receiving mode command
    client.add_ok_response();
    // Connect command
    client.add_ok_response();

    let mut adapter: AdapterType = Adapter::new(client, timer);

    let mut socket = adapter.socket().unwrap();
    let error = adapter
        .connect(&mut socket, SocketAddr::from_str("127.0.0.1:6000").unwrap())
        .unwrap_err();

    assert_eq!(nb::Error::Other(Error::ConnectUnconfirmed), error);
}

#[test]
fn test_send_not_connected() {
    let timer = MockTimer::new();
    let mut client = MockAtatClient::new();

    // Multiple connections command
    client.add_ok_response();

    let mut adapter: AdapterType = Adapter::new(client, timer);
    let mut socket = adapter.socket().unwrap();

    let error = adapter.send(&mut socket, b"test data").unwrap_err();
    assert_eq!(nb::Error::Other(Error::SocketUnconnected), error);
}

#[test]
fn test_send_tx_prepare_error() {
    let timer = MockTimer::new();
    let client = MockAtatClient::new();
    let mut adapter: AdapterType = Adapter::new(client, timer);
    let mut socket = connect_socket(&mut adapter);

    adapter.client.add_error_response();

    let error = adapter.send(&mut socket, b"test data").unwrap_err();
    assert_eq!(nb::Error::Other(Error::TransmissionStartFailed(AtError::Parse)), error);
}

#[test]
fn test_send_tx_prepare_would_block() {
    let timer = MockTimer::new();
    let client = MockAtatClient::new();
    let mut adapter: AdapterType = Adapter::new(client, timer);
    let mut socket = connect_socket(&mut adapter);

    adapter.client.send_would_block(0);

    let error = adapter.send(&mut socket, b"test data").unwrap_err();
    assert_eq!(nb::Error::Other(Error::UnexpectedWouldBlock), error);
}

#[test]
fn test_send_tx_command_would_block() {
    let timer = MockTimer::new();
    let client = MockAtatClient::new();
    let mut adapter: AdapterType = Adapter::new(client, timer);
    let mut socket = connect_socket(&mut adapter);

    // TX prepare command
    adapter.client.add_ok_response();
    // Actual TX command
    adapter.client.send_would_block(1);

    let error = adapter.send(&mut socket, b"test data").unwrap_err();
    assert_eq!(nb::Error::Other(Error::UnexpectedWouldBlock), error);
}

#[test]
fn test_send_timer_start_error() {
    let mut timer = MockTimer::new();
    timer.expect_start().times(1).returning(|duration| {
        assert_eq!(duration, MockTimer::duration_ms(5_000));
        Err(100)
    });

    let client = MockAtatClient::new();
    let mut adapter: AdapterType = Adapter::new(client, timer);
    let mut socket = connect_socket(&mut adapter);

    // TX prepare command
    adapter.client.add_ok_response();
    // Actual TX command
    adapter.client.add_ok_response();

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

    let client = MockAtatClient::new();
    let mut adapter: AdapterType = Adapter::new(client, timer);
    let mut socket = connect_socket(&mut adapter);

    // TX prepare command
    adapter.client.add_ok_response();
    // Actual TX command
    adapter.client.add_ok_response();

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

    let client = MockAtatClient::new();
    let mut adapter: AdapterType = Adapter::new(client, timer);
    let mut socket = connect_socket(&mut adapter);

    // TX prepare command
    adapter.client.add_ok_response();
    // Actual TX command
    adapter.client.add_ok_response();

    adapter.client.expect_reset_calls();

    let error = adapter.send(&mut socket, b"test data").unwrap_err();
    assert_eq!(nb::Error::Other(Error::SendFailed(AtError::Timeout)), error);
    assert_eq!(1, adapter.client.get_reset_call_count());
}

#[test]
fn test_send_byte_count_not_matching() {
    let mut timer = MockTimer::new();
    timer.expect_start().times(1).returning(|_| Ok(()));

    let client = MockAtatClient::new();
    let mut adapter: AdapterType = Adapter::new(client, timer);
    let mut socket = connect_socket(&mut adapter);

    // TX prepare command
    adapter.client.add_ok_response();
    // Actual TX command
    adapter.client.add_ok_response();

    adapter.client.skip_urc(1);
    adapter.client.add_urc_recv_bytes();
    adapter.client.add_urc_send_ok();

    let error = adapter.send(&mut socket, b"test data").unwrap_err();
    assert_eq!(nb::Error::Other(Error::PartialSend), error);
}

#[test]
fn test_send_ok_without_recv_message() {
    let mut timer = MockTimer::new();
    timer.expect_start().times(1).returning(|_| Ok(()));

    let client = MockAtatClient::new();
    let mut adapter: AdapterType = Adapter::new(client, timer);
    let mut socket = connect_socket(&mut adapter);

    // TX prepare command
    adapter.client.add_ok_response();
    // Actual TX command
    adapter.client.add_ok_response();

    adapter.client.skip_urc(1);
    adapter.client.add_urc_send_ok();

    adapter.send(&mut socket, b"test data").unwrap();
}

#[test]
fn test_send_fail_urc_message() {
    let mut timer = MockTimer::new();
    timer.expect_start().times(1).returning(|_| Ok(()));

    let client = MockAtatClient::new();
    let mut adapter: AdapterType = Adapter::new(client, timer);
    let mut socket = connect_socket(&mut adapter);

    // TX prepare command
    adapter.client.add_ok_response();
    // Actual TX command
    adapter.client.add_ok_response();

    adapter.client.skip_urc(1);
    adapter.client.add_urc_send_fail();
    adapter.client.add_urc_recv_bytes();
    adapter.client.expect_reset_calls();

    let error = adapter.send(&mut socket, b"test").unwrap_err();
    assert_eq!(nb::Error::Other(Error::SendFailed(AtError::Error)), error);
    assert_eq!(1, adapter.client.get_reset_call_count())
}

#[test]
fn test_send_error_and_recv_bytes_not_matching() {
    let mut timer = MockTimer::new();
    timer.expect_start().times(1).returning(|_| Ok(()));

    let client = MockAtatClient::new();
    let mut adapter: AdapterType = Adapter::new(client, timer);
    let mut socket = connect_socket(&mut adapter);

    // TX prepare command
    adapter.client.add_ok_response();
    // Actual TX command
    adapter.client.add_ok_response();

    adapter.client.skip_urc(1);
    adapter.client.add_urc_send_fail();
    adapter.client.add_urc_recv_bytes();
    adapter.client.expect_reset_calls();

    let error = adapter.send(&mut socket, b"test data").unwrap_err();
    assert_eq!(nb::Error::Other(Error::SendFailed(AtError::Error)), error);
    assert_eq!(1, adapter.client.get_reset_call_count());
}

#[test]
fn test_send_correct_commands() {
    let mut timer = MockTimer::new();
    timer.expect_start().times(1).returning(|_| Ok(()));

    let client = MockAtatClient::new();
    let mut adapter: AdapterType = Adapter::new(client, timer);
    let mut socket = connect_socket(&mut adapter);

    // TX prepare command
    adapter.client.add_ok_response();
    // Actual TX command
    adapter.client.add_ok_response();

    adapter.client.skip_urc(1);
    adapter.client.add_urc_send_ok();
    adapter.client.add_urc_recv_bytes();

    let sent_bytes = adapter.send(&mut socket, b"test").unwrap();
    assert_eq!(4, sent_bytes);

    let commands = adapter.client.get_commands_as_strings();
    assert_eq!(5, commands.len());
    assert_eq!("AT+CIPSEND=0,4\r\n".to_string(), commands[3]);
    assert_eq!("test".to_string(), commands[4]);
}

#[test]
fn test_send_multiple_calls_urc_status_reset() {
    let mut timer = MockTimer::new();
    timer.expect_start().times(2).returning(|_| Ok(()));

    let client = MockAtatClient::new();
    let mut adapter: AdapterType = Adapter::new(client, timer);
    let mut socket = connect_socket(&mut adapter);

    adapter.client.add_ok_response();
    adapter.client.add_ok_response();
    adapter.client.skip_urc(1);
    adapter.client.add_urc_send_fail();
    adapter.client.add_urc_recv_bytes();
    adapter.client.expect_reset_calls();

    assert!(adapter.send(&mut socket, b"test").is_err());
    assert_eq!(1, adapter.client.get_reset_call_count());

    adapter.client.add_ok_response();
    adapter.client.add_ok_response();
    adapter.client.skip_urc(1);
    adapter.client.add_urc_send_ok();

    let sent_bytes = adapter.send(&mut socket, b"test data").unwrap();
    assert_eq!(9, sent_bytes);

    let commands = adapter.client.get_commands_as_strings();
    assert_eq!(7, commands.len());
    assert_eq!("AT+CIPSEND=0,9\r\n".to_string(), commands[5]);
    assert_eq!("test data".to_string(), commands[6]);
}

#[test]
fn test_send_chunks() {
    let mut timer = MockTimer::new();
    timer.expect_start().times(2).returning(|_| Ok(()));

    let client = MockAtatClient::new();
    let mut adapter: AdapterType = Adapter::new(client, timer);
    let mut socket = connect_socket(&mut adapter);

    // First TX prepare command
    adapter.client.add_ok_response();
    // First actual TX command
    adapter.client.add_ok_response();

    // Second TX prepare command
    adapter.client.add_ok_response();
    // Second actual TX command
    adapter.client.add_ok_response();

    adapter.client.skip_urc(1);
    adapter.client.throttle_urc();
    adapter.client.add_urc_send_ok();
    adapter.client.add_urc_send_ok();

    let mut buffer = vec![b'A'; 256];
    buffer.extend_from_slice(b"second message");

    let sent_bytes = adapter.send(&mut socket, buffer.as_slice()).unwrap();
    assert_eq!(270, sent_bytes);

    let commands = adapter.client.get_commands_as_strings();
    assert_eq!(7, commands.len());
    assert_eq!("AT+CIPSEND=0,256\r\n".to_string(), commands[3]);
    assert_eq!(String::from_utf8(vec![b'A'; 256]).unwrap(), commands[4]);
    assert_eq!("AT+CIPSEND=0,14\r\n".to_string(), commands[5]);
    assert_eq!("second message".to_string(), commands[6]);
}

#[test]
fn test_receive_no_data_available() {
    let timer = MockTimer::new();
    let client = MockAtatClient::new();
    let mut adapter: AdapterType = Adapter::new(client, timer);
    let mut socket = connect_socket(&mut adapter);

    // Other socket
    adapter.client.add_urc_message(b"+IPD,3,256\r\n");

    let mut buffer = [0x0; 32];
    let length = adapter.receive(&mut socket, &mut buffer).unwrap();

    assert_eq!(0, length);
    assert_eq!([0x0; 32], buffer);
}

#[test]
fn test_receive_receive_command_failed() {
    let timer = MockTimer::new();
    let client = MockAtatClient::new();
    let mut adapter: AdapterType = Adapter::new(client, timer);
    let mut socket = connect_socket(&mut adapter);

    adapter.client.add_error_response();
    adapter.client.add_urc_message(b"+IPD,0,256\r\n");

    let mut buffer = [0x0; 32];
    let error = adapter.receive(&mut socket, &mut buffer).unwrap_err();
    assert_eq!(nb::Error::Other(Error::ReceiveFailed(AtError::Parse)), error);
}

#[test]
fn test_receive_receive_command_would_block() {
    let timer = MockTimer::new();
    let client = MockAtatClient::new();
    let mut adapter: AdapterType = Adapter::new(client, timer);
    let mut socket = connect_socket(&mut adapter);

    adapter.client.send_would_block(0);
    adapter.client.add_urc_message(b"+IPD,0,256\r\n");

    let mut buffer = [0x0; 32];
    let error = adapter.receive(&mut socket, &mut buffer).unwrap_err();
    assert_eq!(nb::Error::Other(Error::UnexpectedWouldBlock), error);
}

#[test]
fn test_receive_no_data_received() {
    let timer = MockTimer::new();
    let client = MockAtatClient::new();
    let mut adapter: AdapterType = Adapter::new(client, timer);
    let mut socket = connect_socket(&mut adapter);

    adapter.client.add_urc_message(b"+IPD,0,256\r\n");
    adapter.client.add_ok_response();

    let mut buffer = [0x0; 32];
    let error = adapter.receive(&mut socket, &mut buffer).unwrap_err();
    assert_eq!(nb::Error::Other(Error::ReceiveFailed(AtError::InvalidResponse)), error);
}

#[test]
fn test_receive_correct_command() {
    let timer = MockTimer::new();
    let client = MockAtatClient::new();
    let mut adapter: AdapterType = Adapter::new(client, timer);
    let mut socket = connect_socket(&mut adapter);

    adapter.client.add_urc_message(b"+IPD,0,4\r\n");
    adapter.client.add_ok_response();

    adapter.client.throttle_urc();
    adapter.client.add_urc_message(b"+CIPRECVDATA,4:aaaa");

    let mut buffer = [b' '; 16];
    adapter.receive(&mut socket, &mut buffer).unwrap();

    let commands = adapter.client.get_commands_as_strings();
    assert_eq!(4, commands.len());
    assert_eq!("AT+CIPRECVDATA=0,4\r\n".to_string(), commands[3]);
}

#[test]
fn test_receive_data_received_buffer_bigger_then_block_size() {
    let timer = MockTimer::new();
    let client = MockAtatClient::new();
    let mut adapter: AdapterType = Adapter::new(client, timer);
    let mut socket = connect_socket(&mut adapter);

    adapter.client.add_urc_message(b"+IPD,0,10\r\n");
    adapter.client.add_ok_response();
    adapter.client.add_ok_response();
    adapter.client.add_ok_response();

    adapter.client.throttle_urc();
    adapter.client.add_urc_message(b"+CIPRECVDATA,4:aaaa");
    adapter.client.add_urc_message(b"+CIPRECVDATA,4:bbbb");
    adapter.client.add_urc_message(b"+CIPRECVDATA,2:cc");

    let mut buffer = [b' '; 16];
    let length = adapter.receive(&mut socket, &mut buffer).unwrap();

    assert_eq!(10, length);
    assert_eq!(b"aaaabbbbcc      ", &buffer);
}

#[test]
fn test_receive_data_received_buffer_smaller_then_block_size() {
    let timer = MockTimer::new();
    let client = MockAtatClient::new();
    let mut adapter: AdapterType = Adapter::new(client, timer);
    let mut socket = connect_socket(&mut adapter);

    adapter.client.add_urc_message(b"+IPD,0,5\r\n");
    adapter.client.add_ok_response();
    adapter.client.add_ok_response();
    adapter.client.add_ok_response();

    adapter.client.throttle_urc();
    adapter.client.add_urc_message(b"+CIPRECVDATA,2:aa");
    adapter.client.add_urc_message(b"+CIPRECVDATA,2:bb");
    adapter.client.add_urc_message(b"+CIPRECVDATA,1:c");

    let mut buffer = [b' '; 2];
    let length = adapter.receive(&mut socket, &mut buffer).unwrap();
    assert_eq!(2, length);
    assert_eq!(b"aa", &buffer);

    let mut buffer = [b' '; 2];
    let length = adapter.receive(&mut socket, &mut buffer).unwrap();
    assert_eq!(2, length);
    assert_eq!(b"bb", &buffer);

    let mut buffer = [b' '; 2];
    let length = adapter.receive(&mut socket, &mut buffer).unwrap();
    assert_eq!(1, length);
    assert_eq!(b"c ", &buffer);
}

#[test]
fn test_receive_data_received_less_data_received_then_requested() {
    let timer = MockTimer::new();
    let client = MockAtatClient::new();
    let mut adapter: AdapterType = Adapter::new(client, timer);
    let mut socket = connect_socket(&mut adapter);

    adapter.client.add_urc_message(b"+IPD,0,10\r\n");
    adapter.client.add_ok_response();
    adapter.client.add_ok_response();
    adapter.client.add_ok_response();
    adapter.client.add_ok_response();

    adapter.client.throttle_urc();
    // 4 bytes requested, but just two bytes received
    adapter.client.add_urc_message(b"+CIPRECVDATA,2:aa");
    adapter.client.add_urc_message(b"+CIPRECVDATA,2:aa");
    adapter.client.add_urc_message(b"+CIPRECVDATA,4:bbbb");
    adapter.client.add_urc_message(b"+CIPRECVDATA,2:cc");

    let mut buffer = [b' '; 16];
    let length = adapter.receive(&mut socket, &mut buffer).unwrap();

    assert_eq!(10, length);
    assert_eq!(b"aaaabbbbcc      ", &buffer);
}

/// This can just happen if ESP-AT sends more data then requested, which is a protocol violation.
#[test]
fn test_receive_data_received_more_data_received_then_block_size() {
    let timer = MockTimer::new();
    let client = MockAtatClient::new();
    let mut adapter: AdapterType = Adapter::new(client, timer);
    let mut socket = connect_socket(&mut adapter);

    adapter.client.add_urc_message(b"+IPD,0,5\r\n");
    adapter.client.add_ok_response();

    adapter.client.throttle_urc();
    // 4 bytes requested, but 5 received
    adapter.client.add_urc_message(b"+CIPRECVDATA,5:aaaaa");

    let mut buffer = [b' '; 16];
    let error = adapter.receive(&mut socket, &mut buffer).unwrap_err();
    assert_eq!(nb::Error::Other(Error::ReceiveFailed(AtError::InvalidResponse)), error);
}

/// This can just happen if ESP-AT sends more data then requested, which is a protocol violation.
#[test]
fn test_receive_data_received_buffer_overflow() {
    let timer = MockTimer::new();
    let client = MockAtatClient::new();
    let mut adapter: AdapterType = Adapter::new(client, timer);
    let mut socket = connect_socket(&mut adapter);

    adapter.client.add_urc_message(b"+IPD,0,5\r\n");
    adapter.client.add_ok_response();
    adapter.client.add_ok_response();

    adapter.client.throttle_urc();
    adapter.client.add_urc_message(b"+CIPRECVDATA,4:aaaa");
    adapter.client.add_urc_message(b"+CIPRECVDATA,2:bb");

    let mut buffer = [b' '; 5];
    let error = adapter.receive(&mut socket, &mut buffer).unwrap_err();
    assert_eq!(nb::Error::Other(Error::ReceiveOverflow), error);
}

/// Helper for opening & connecting a socket
fn connect_socket(adapter: &mut AdapterType) -> Socket {
    // Receiving socket
    adapter.client.add_ok_response();

    // Connecting socket
    adapter.client.add_ok_response();
    adapter.client.add_ok_response();
    adapter.client.skip_urc(1);
    adapter.client.add_urc_first_socket_connected();

    let mut socket = adapter.socket().unwrap();
    adapter
        .connect(&mut socket, SocketAddr::from_str("127.0.0.1:5000").unwrap())
        .unwrap();

    socket
}
