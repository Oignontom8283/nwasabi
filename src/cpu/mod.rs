//! Processeur ARM Cortex-M7 (ARMv7E-M, Thumb-2, virgule flottante FPv5-D16 simple précision).

mod bus;
mod regs;

pub use bus::{Bus, BusFault, Width};
pub use regs::Registers;
