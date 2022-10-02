#![cfg_attr(not(test), no_std)]
#![cfg_attr(feature = "strict", deny(warnings))]

extern crate alloc;

pub(crate) mod commands;
pub(crate) mod responses;
pub mod stack;
pub mod urc;
pub mod wifi;

#[cfg(test)]
mod tests;
