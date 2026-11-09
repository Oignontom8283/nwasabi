//! L'architecture que GDB voit : un Cortex-M, avec ses 17 registres (r0 à r12, sp, lr, pc, xpsr), décrits par le fichier XML de
//! la cible. `gdbstub_arch` n'a pas de profil M : celui-ci est écrit ici.

use std::num::NonZeroUsize;

use gdbstub::arch::{Arch, RegId, Registers};

use super::paquets::TARGET_XML;

/// Nombre de registres, dans l'ordre du paquet `g` : r0 à r12, sp, lr, pc, xpsr.
pub(super) const REGISTER_COUNT: usize = 17;

pub(super) enum CortexM {}

impl Arch for CortexM {
    type Usize = u32;
    type Registers = CoreRegisters;
    type RegId = CoreRegister;
    /// Le « genre » de point d'arrêt donné par GDB (la taille de l'instruction : 2 ou 4) ne change rien ici.
    type BreakpointKind = usize;

    fn target_description_xml() -> Option<&'static str> {
        Some(TARGET_XML)
    }
}

/// Les registres du cœur, dans l'ordre du paquet `g`.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub(super) struct CoreRegisters(pub [u32; REGISTER_COUNT]);

impl Registers for CoreRegisters {
    type ProgramCounter = u32;

    fn pc(&self) -> u32 {
        self.0[15]
    }

    fn gdb_serialize(&self, mut write_byte: impl FnMut(Option<u8>)) {
        for value in self.0 {
            for byte in value.to_le_bytes() {
                write_byte(Some(byte));
            }
        }
    }

    fn gdb_deserialize(&mut self, bytes: &[u8]) -> Result<(), ()> {
        if bytes.len() < REGISTER_COUNT * 4 {
            return Err(());
        }
        for (register, chunk) in self.0.iter_mut().zip(bytes.chunks_exact(4)) {
            *register = u32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
        }
        Ok(())
    }
}

/// Un registre, par son numéro (0 à 16).
#[derive(Debug)]
pub(super) struct CoreRegister(pub usize);

impl RegId for CoreRegister {
    fn from_raw_id(id: usize) -> Option<(Self, Option<NonZeroUsize>)> {
        (id < REGISTER_COUNT).then(|| (Self(id), NonZeroUsize::new(4)))
    }
}
