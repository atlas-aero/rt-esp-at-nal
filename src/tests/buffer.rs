use crate::stack::{Buffer, Error};
use heapless::Vec;

#[test]
fn test_get_next_length_empty() {
    // Chunk size
    let mut inner = [0x0; 32];
    let buffer: Buffer<16> = Buffer::new(&mut inner);
    assert_eq!(16, buffer.get_next_length());

    // Buffer space
    let mut inner = [0x0; 8];
    let buffer: Buffer<16> = Buffer::new(&mut inner);
    assert_eq!(8, buffer.get_next_length());

    // Equal
    let mut inner = [0x0; 10];
    let buffer: Buffer<10> = Buffer::new(&mut inner);
    assert_eq!(10, buffer.get_next_length());
}

#[test]
fn test_get_next_length_partial_filled() {
    let mut inner = [0x0; 30];
    let mut buffer: Buffer<16> = Buffer::new(&mut inner);
    assert_eq!(16, buffer.get_next_length());

    buffer.append(Vec::from_slice(&[0x0; 16]).unwrap()).unwrap();
    assert_eq!(14, buffer.get_next_length());
}

#[test]
fn test_append_correct_result() {
    let mut inner = [0x0; 19];
    let mut buffer: Buffer<16> = Buffer::new(&mut inner);

    buffer.append(Vec::from_slice(b"correct ").unwrap()).unwrap();
    buffer.append(Vec::from_slice(b"test ").unwrap()).unwrap();
    buffer.append(Vec::from_slice(b"result").unwrap()).unwrap();

    assert_eq!(b"correct test result", &inner);
}

#[test]
fn test_append_partial() {
    let mut inner = [0x20; 32];
    let mut buffer: Buffer<16> = Buffer::new(&mut inner);

    buffer.append(Vec::from_slice(b"correct ").unwrap()).unwrap();
    buffer.append(Vec::from_slice(b"test ").unwrap()).unwrap();
    buffer.append(Vec::from_slice(b"result").unwrap()).unwrap();

    assert_eq!(b"correct test result             ", &inner);
}

#[test]
fn test_append_not_enough_space() {
    let mut inner = [0x20; 8];
    let mut buffer: Buffer<16> = Buffer::new(&mut inner);

    buffer.append(Vec::from_slice(b"7 bytes").unwrap()).unwrap();
    let error = buffer.append(Vec::from_slice(b"ab").unwrap()).unwrap_err();
    assert_eq!(Error::ReceiveOverflow, error);
}

#[test]
fn test_is_full_zero_size() {
    let mut inner = [0x0; 0];
    let buffer: Buffer<16> = Buffer::new(&mut inner);

    assert!(buffer.is_full());
}

#[test]
fn test_is_full_false() {
    let mut inner = [0x0; 8];
    let mut buffer: Buffer<16> = Buffer::new(&mut inner);
    buffer.append(Vec::from_slice(b"7 bytes").unwrap()).unwrap();

    assert!(!buffer.is_full());
}

#[test]
fn test_is_full_true() {
    let mut inner = [0x0; 7];
    let mut buffer: Buffer<16> = Buffer::new(&mut inner);
    buffer.append(Vec::from_slice(b"7 bytes").unwrap()).unwrap();

    assert!(buffer.is_full());
}

#[test]
fn test_len_empty() {
    let mut inner = [0x0; 10];
    let buffer: Buffer<16> = Buffer::new(&mut inner);

    assert_eq!(0, buffer.len());
}

#[test]
fn test_len_partial_filled() {
    let mut inner = [0x0; 10];
    let mut buffer: Buffer<16> = Buffer::new(&mut inner);
    buffer.append(Vec::from_slice(b"7 bytes").unwrap()).unwrap();

    assert_eq!(7, buffer.len());
}

#[test]
fn test_len_partial_full() {
    let mut inner = [0x0; 10];
    let mut buffer: Buffer<16> = Buffer::new(&mut inner);
    buffer.append(Vec::from_slice(b"10   bytes").unwrap()).unwrap();

    assert_eq!(10, buffer.len());
}
