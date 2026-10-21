//! Le processeur lit et écrit la mémoire à travers le trait `cpu::Bus`.

use super::{Memory, access};
use crate::cpu::{Bus, BusFault, FaultKind, Width};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Read,
    Write,
    Fetch,
}

impl Memory {
    /// Un accès du programme : vérifie la zone et les droits, marque les lignes touchées.
    fn access<const N: usize>(
        &mut self,
        address: u32,
        kind: Kind,
        data: Option<[u8; N]>,
    ) -> Result<[u8; N], BusFault> {
        let width = match N {
            1 => Width::U8,
            2 => Width::U16,
            _ => Width::U32,
        };
        let fault = |kind_of_fault| BusFault {
            address,
            write: kind == Kind::Write,
            width,
            kind: kind_of_fault,
        };
        let index = self
            .find(address, N as u32)
            .ok_or_else(|| fault(FaultKind::Unmapped))?;
        let region = &mut self.regions[index];
        let allowed = match kind {
            Kind::Read => region.spec.perms.read,
            Kind::Write => region.spec.perms.write,
            Kind::Fetch => region.spec.perms.execute,
        };
        if !allowed {
            return Err(fault(FaultKind::AccessDenied));
        }
        let offset = address - region.spec.base;
        let mut bytes = [0u8; N];
        match kind {
            Kind::Write => {
                let data = data.expect("une écriture porte des données");
                self.tick += 1;
                let tick = self.tick;
                let region = &mut self.regions[index];
                region.copy_in(offset, &data);
                region.mark_access(offset, N as u32, access::WRITE);
                region.stamp(offset, N as u32, tick);
            }
            Kind::Read | Kind::Fetch => {
                region.copy_out(offset, &mut bytes);
                let flag = if kind == Kind::Read {
                    access::READ
                } else {
                    access::EXECUTE
                };
                region.mark_access(offset, N as u32, flag);
            }
        }
        if !self.watches.is_empty() && kind != Kind::Fetch {
            let value = match kind {
                Kind::Write => data.map_or(0, le),
                _ => le(bytes),
            };
            self.watches
                .check(address, N as u8, kind == Kind::Write, value);
        }
        Ok(bytes)
    }
}

/// Valeur little-endian de 1 à 4 octets.
fn le<const N: usize>(bytes: [u8; N]) -> u32 {
    bytes.iter().rev().fold(0, |v, &b| v << 8 | u32::from(b))
}

impl Bus for Memory {
    fn read8(&mut self, address: u32) -> Result<u8, BusFault> {
        Ok(self.access::<1>(address, Kind::Read, None)?[0])
    }

    fn read16(&mut self, address: u32) -> Result<u16, BusFault> {
        Ok(u16::from_le_bytes(self.access::<2>(
            address,
            Kind::Read,
            None,
        )?))
    }

    fn read32(&mut self, address: u32) -> Result<u32, BusFault> {
        Ok(u32::from_le_bytes(self.access::<4>(
            address,
            Kind::Read,
            None,
        )?))
    }

    fn write8(&mut self, address: u32, value: u8) -> Result<(), BusFault> {
        self.access::<1>(address, Kind::Write, Some([value]))
            .map(|_| ())
    }

    fn write16(&mut self, address: u32, value: u16) -> Result<(), BusFault> {
        self.access::<2>(address, Kind::Write, Some(value.to_le_bytes()))
            .map(|_| ())
    }

    fn write32(&mut self, address: u32, value: u32) -> Result<(), BusFault> {
        self.access::<4>(address, Kind::Write, Some(value.to_le_bytes()))
            .map(|_| ())
    }

    fn fetch16(&mut self, address: u32) -> Result<u16, BusFault> {
        Ok(u16::from_le_bytes(self.access::<2>(
            address,
            Kind::Fetch,
            None,
        )?))
    }
}
