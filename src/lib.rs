#![cfg_attr(not(test), no_std)]
#![cfg_attr(feature = "strict", deny(warnings))]

extern crate alloc;

pub mod adapter;
pub(crate) mod commands;
pub(crate) mod responses;
pub mod stack;
pub mod urc;

#[cfg(test)]
mod tests;
