use atat::atat_derive::AtatResp;

/// Commands which gets just responded by OK
#[derive(Clone, AtatResp)]
pub struct NoResponse;
