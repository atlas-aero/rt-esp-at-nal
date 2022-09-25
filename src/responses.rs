use atat::atat_derive::AtatResp;
use atat::AtatResp as Response;

/// Commands which gets just responded by OK
#[derive(Clone, AtatResp)]
pub struct NoResponse;

#[derive(Clone, Debug)]
pub struct WifiConnectResponse {}

impl Response for WifiConnectResponse {}
