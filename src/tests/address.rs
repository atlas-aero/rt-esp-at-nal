use crate::tests::mock::{MockAtatClient, MockTimer};
use crate::wifi::WifiAdapter;
use crate::wifi::{Adapter, AddressErrors};
use alloc::string::ToString;
use atat::Error as AtError;
type AdapterType = Adapter<MockAtatClient, MockTimer, 1_000_000, 256, 64>;

#[test]
fn test_all_addresses() {
    let client = MockAtatClient::new();
    let timer = MockTimer::new();
    let mut adapter: AdapterType = Adapter::new(client, timer);

    adapter.client.add_response(b"+CIFSR:STAIP,\"10.0.0.181\"\r\n+CIFSR:STAIP6LL,\"fe80::e6ee:e64e:84c:a745\"\r\n+CIFSR:STAMAC,\"10:fe:ed:05:ba:50\"\r\n+CIFSR:STAIP6GL,\"2a02:810d:1340:2df5:68e1:704d:4a72:656a\"\r\n");

    let address = adapter.get_address().unwrap();
    assert_eq!("10:fe:ed:05:ba:50", address.mac.unwrap().as_str());
    assert_eq!("10.0.0.181", address.ipv4.unwrap().to_string());
    assert_eq!("fe80::e6ee:e64e:84c:a745", address.ipv6_link_local.unwrap().to_string());
    assert_eq!(
        "2a02:810d:1340:2df5:68e1:704d:4a72:656a",
        address.ipv6_global.unwrap().to_string()
    );
}

#[test]
fn test_ipv6_missing() {
    let client = MockAtatClient::new();
    let timer = MockTimer::new();
    let mut adapter: AdapterType = Adapter::new(client, timer);

    adapter
        .client
        .add_response(b"+CIFSR:STAIP,\"10.0.0.181\"\r\n+CIFSR:STAMAC,\"10:fe:ed:05:ba:50\"\r\n");

    let address = adapter.get_address().unwrap();
    assert_eq!("10:fe:ed:05:ba:50", address.mac.unwrap().as_str());
    assert_eq!("10.0.0.181", address.ipv4.unwrap().to_string());
    assert!(address.ipv6_global.is_none());
    assert!(address.ipv6_link_local.is_none());
}

#[test]
fn test_ipv4_missing() {
    let client = MockAtatClient::new();
    let timer = MockTimer::new();
    let mut adapter: AdapterType = Adapter::new(client, timer);

    adapter.client.add_response(b"+CIFSR:STAIP6LL,\"fe80::e6ee:e64e:84c:a745\"\r\n+CIFSR:STAMAC,\"10:fe:ed:05:ba:50\"\r\n+CIFSR:STAIP6GL,\"2a02:810d:1340:2df5:68e1:704d:4a72:656a\"\r\n");

    let address = adapter.get_address().unwrap();
    assert_eq!("10:fe:ed:05:ba:50", address.mac.unwrap().as_str());
    assert!(address.ipv4.is_none());
    assert_eq!("fe80::e6ee:e64e:84c:a745", address.ipv6_link_local.unwrap().to_string());
    assert_eq!(
        "2a02:810d:1340:2df5:68e1:704d:4a72:656a",
        address.ipv6_global.unwrap().to_string()
    );
}

#[test]
fn test_missing_mac() {
    let client = MockAtatClient::new();
    let timer = MockTimer::new();
    let mut adapter: AdapterType = Adapter::new(client, timer);

    adapter.client.add_response(b"+CIFSR:STAIP,\"10.0.0.181\"\r\n+CIFSR:STAIP6LL,\"fe80::e6ee:e64e:84c:a745\"\r\n+CIFSR:STAIP6GL,\"2a02:810d:1340:2df5:68e1:704d:4a72:656a\"\r\n");

    let address = adapter.get_address().unwrap();
    assert!(address.mac.is_none());
    assert_eq!("10.0.0.181", address.ipv4.unwrap().to_string());
    assert_eq!("fe80::e6ee:e64e:84c:a745", address.ipv6_link_local.unwrap().to_string());
    assert_eq!(
        "2a02:810d:1340:2df5:68e1:704d:4a72:656a",
        address.ipv6_global.unwrap().to_string()
    );
}

#[test]
fn test_unknown_type() {
    let client = MockAtatClient::new();
    let timer = MockTimer::new();
    let mut adapter: AdapterType = Adapter::new(client, timer);

    adapter
        .client
        .add_response(b"+CIFSR:STAIP,\"10.0.0.181\"\r\n+CIFSR:MAGCIC,\"123\"\r\n");

    let address = adapter.get_address().unwrap();
    assert_eq!("10.0.0.181", address.ipv4.unwrap().to_string());
    assert!(address.ipv6_global.is_none());
    assert!(address.ipv6_link_local.is_none());
    assert!(address.mac.is_none());
}

#[test]
fn test_ipv4_parse_error() {
    let client = MockAtatClient::new();
    let timer = MockTimer::new();
    let mut adapter: AdapterType = Adapter::new(client, timer);

    adapter.client.add_response(b"+CIFSR:STAIP,\"10.0.0.0.1\"\r\n");

    assert_eq!(AddressErrors::AddressParseError, adapter.get_address().unwrap_err());
}

#[test]
fn test_link_local_ipv6_parse_error() {
    let client = MockAtatClient::new();
    let timer = MockTimer::new();
    let mut adapter: AdapterType = Adapter::new(client, timer);

    adapter.client.add_response(b"+CIFSR:STAIP6LL,\"zzz\"\r\n");

    assert_eq!(AddressErrors::AddressParseError, adapter.get_address().unwrap_err());
}

#[test]
fn test_global_ipv6_parse_error() {
    let client = MockAtatClient::new();
    let timer = MockTimer::new();
    let mut adapter: AdapterType = Adapter::new(client, timer);

    adapter.client.add_response(b"+CIFSR:STAIP6GL,\"123\"\r\n");

    assert_eq!(AddressErrors::AddressParseError, adapter.get_address().unwrap_err());
}

#[test]
fn test_mac_to_long() {
    let client = MockAtatClient::new();
    let timer = MockTimer::new();
    let mut adapter: AdapterType = Adapter::new(client, timer);

    adapter.client.add_response(b"+CIFSR:STAMAC,\"10:fe:ed:05:ba:50_\"\r\n");

    assert_eq!(AddressErrors::AddressParseError, adapter.get_address().unwrap_err());
}

#[test]
fn test_would_block() {
    let client = MockAtatClient::new();
    let timer = MockTimer::new();
    let mut adapter: AdapterType = Adapter::new(client, timer);

    adapter.client.send_would_block(0);

    assert_eq!(AddressErrors::UnexpectedWouldBlock, adapter.get_address().unwrap_err());
}

#[test]
fn test_command_error() {
    let client = MockAtatClient::new();
    let timer = MockTimer::new();
    let mut adapter: AdapterType = Adapter::new(client, timer);

    adapter.client.add_error_response();

    assert_eq!(
        AddressErrors::CommandError(AtError::Parse),
        adapter.get_address().unwrap_err()
    );
}
