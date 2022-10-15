//! # no_std ESP-AT network layer
//!
//! Network layer implementation/client for [ESP-AT](https://docs.espressif.com/projects/esp-at/)
//! implementing [embedded-nal](embedded_nal) based on [ATAT](https://crates.io/crates/atat).
//!
//! Currently this crates offers the following features
//! * Joining an WIFI access point, s. [wifi module](crate::wifi)
//! * TCP client stack (multi socket), s. [stack module](crate::stack)
//!
//! ## Setup
//! This crates is based on [ATAT](atat) and requires a AtClient instance.
//! s. [examples](https://github.com/BlackbirdHQ/atat/tree/master/atat/examples).
//!
//! ## Example
//!
//! ````
//! use core::str::FromStr;
//! use embedded_nal::{SocketAddr, TcpClientStack};
//! use esp_at_nal::example::ExampleTimer;
//! use esp_at_nal::wifi::{Adapter, WifiAdapter};
//! use crate::esp_at_nal::example::ExampleAtClient as AtClient;
//!
//! let client = AtClient::default();
//! // Creating adapter with 1024 bytes TX and 256 RX block size
//! let mut adapter: Adapter<_, _, 1_000_000, 1024, 256> = Adapter::new(client, ExampleTimer::default());
//!
//! // Joining WIFI access point
//! let state = adapter.join("test_wifi", "secret").unwrap();
//! assert!(state.connected);
//!
//! // Creating a TCP connection
//! let mut  socket = adapter.socket().unwrap();
//! adapter.connect(&mut socket, SocketAddr::from_str("10.0.0.1:21").unwrap()).unwrap();
//!
//! // Sending some data
//! adapter.send(&mut socket, b"hallo!").unwrap();
//! ````
#![cfg_attr(not(test), no_std)]
#![cfg_attr(feature = "strict", deny(warnings))]

#[cfg(test)]
extern crate alloc;

pub(crate) mod commands;
pub mod example;
pub(crate) mod responses;
pub mod stack;
pub mod urc;
pub mod wifi;

#[cfg(test)]
mod tests;
