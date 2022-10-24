# no_std ESP-AT network layer

[![License](https://img.shields.io/badge/license-MIT-blue.svg)](https://opensource.org/licenses/MIT)
[![License](https://img.shields.io/badge/License-Apache%202.0-blue.svg)](https://opensource.org/licenses/Apache-2.0)
[![Crates.io](https://img.shields.io/crates/v/esp-at-nal.svg)](https://crates.io/crates/esp-at-nal)
[![Actions Status](https://github.com/pegasus-aero/rt-esp-at-nal/workflows/QA/badge.svg)](http://github.com/pegasus-aero/rt-esp-at-nal/actions)

Network layer implementation/client for [ESP-AT](https://docs.espressif.com/projects/esp-at/) implementing [embedded-nal](https://crates.io/crates/embedded-nal) based on [ATAT](https://crates.io/crates/atat).

Currently, this crates offers the following features
* Joining an WIFI access point, s. [wifi module](https://docs.rs/esp-at-nal/latest/esp_at_nal/wifi/index.html)
* TCP client stack (multi socket), s. [stack module](https://docs.rs/esp-at-nal/latest/esp_at_nal/stack/index.html)

## Example

Here's a simple example using a mocked AtClient:

````rust
use std::str::FromStr;
use embedded_nal::{SocketAddr, TcpClientStack};
use esp_at_nal::example::ExampleTimer;
use esp_at_nal::wifi::{Adapter, WifiAdapter};
use crate::esp_at_nal::example::ExampleAtClient as AtClient;

let client = AtClient::default();
// Creating adapter with 1024 bytes TX and 256 RX block size
let mut adapter: Adapter<_, _, 1_000_000, 1024, 256> = Adapter::new(client, ExampleTimer::default());

// Joining WIFI access point
let state = adapter.join("test_wifi", "secret").unwrap();
assert!(state.connected);

// Creating a TCP connection
let mut  socket = adapter.socket().unwrap();
adapter.connect(&mut socket, SocketAddr::from_str("10.0.0.1:21").unwrap()).unwrap();

// Sending some data
adapter.send(&mut socket, b"hallo!").unwrap();
````

To see a real-world example that runs on Linux, check out `examples/linux.rs`:

    # For logging
    export RUST_LOG=trace

    cargo run --example linux --features "atat/log" -- \
        /dev/ttyUSB0 115200 mywifi hellopasswd123

## Development

Any form of support is greatly appreciated. Feel free to create issues and PRs.
See [DEVELOPMENT](DEVELOPMENT.md) for more details.

## License

Licensed under either of

* Apache License, Version 2.0, (LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0)
* MIT license (LICENSE-MIT or http://opensource.org/licenses/MIT)
  at your option.

Each contributor agrees that his/her contribution covers both licenses.
