use atat::atat_derive::AtatResp;
use atat::heapless::String;
use atat::heapless_bytes::Bytes;

/// Commands which gets just responded by OK
#[derive(Clone, AtatResp)]
pub struct NoResponse;

/// Single line response of CIFSR command
#[derive(Clone, AtatResp, Debug)]
pub struct LocalAddressResponse {
    /// Address type
    /// * STAIP: Local IPv4 address
    /// * STAIP6LL: Link local IPv6 address
    /// * STAIP6GL: Global IPv6 address
    /// * STAMAC: Local MAC address
    pub address_type: Bytes<8>,

    /// String encoded address
    pub address: String<64>,
}
