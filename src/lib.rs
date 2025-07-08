#![no_std]
#![feature(const_option)]
#![feature(const_nonnull_new)]

mod pl011;

pub use pl011::Uart;
