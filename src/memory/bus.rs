//! Le processeur lit et écrit la mémoire à travers le trait `cpu::Bus`.

use super::{Memory, RegionKind, access};
use crate::cpu::{Bus, BusFault, FaultKind, Width};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Read,
    Write,
}

/// Nombre d'octets d'un accès.
fn width_bytes(width: Width) -> u32 {
    match width {
        Width::U8 => 1,
        Width::U16 => 2,
        Width::U32 => 4,
    }
}

impl Memory {
    fn fault(address: u32, write: bool, width: Width, kind: FaultKind) -> BusFault {
        BusFault {
            address,
            write,
            width,
            kind,
        }
    }

    /// Lecture (ou chargement d'instruction) du programme : vérifie la zone et les droits, marque
    /// la ou les lignes touchées.
    #[inline]
    fn read_access<const N: usize>(
        &mut self,
        address: u32,
        fetch: bool,
    ) -> Result<[u8; N], BusFault> {
        let width = Self::width(N);
        let Some(index) = self.find(address, N as u32) else {
            return Err(Self::fault(address, false, width, FaultKind::Unmapped));
        };
        let region = &mut self.regions[index];
        let perms = region.spec.perms;
        if !(if fetch { perms.execute } else { perms.read }) {
            return Err(Self::fault(address, false, width, FaultKind::AccessDenied));
        }
        let offset = address - region.spec.base;
        let bytes = region.read_small::<N>(offset);
        region.mark_small(
            offset,
            N as u32,
            if fetch { access::EXECUTE } else { access::READ },
        );
        if !fetch && !self.watches.is_empty() {
            self.watches.check(address, N as u8, false, le(bytes));
        }
        Ok(bytes)
    }

    /// Écriture du programme.
    #[inline]
    fn write_access<const N: usize>(
        &mut self,
        address: u32,
        data: [u8; N],
    ) -> Result<(), BusFault> {
        let width = Self::width(N);
        let Some(index) = self.find(address, N as u32) else {
            return Err(Self::fault(address, true, width, FaultKind::Unmapped));
        };
        let region = &mut self.regions[index];
        if !region.spec.perms.write {
            return Err(Self::fault(address, true, width, FaultKind::AccessDenied));
        }
        let offset = address - region.spec.base;
        self.tick += 1;
        if region.access[(offset / super::LINE_SIZE) as usize] & access::EXECUTE != 0
            || region.has_access(offset, N as u32, access::EXECUTE)
        {
            self.code_epoch += 1;
        }
        region.write_small::<N>(offset, data);
        region.mark_small(offset, N as u32, access::WRITE);
        region.stamp_small(offset, N as u32, self.tick);
        if !self.watches.is_empty() {
            self.watches.check(address, N as u8, true, le(data));
        }
        Ok(())
    }

    fn width(n: usize) -> Width {
        match n {
            1 => Width::U8,
            2 => Width::U16,
            _ => Width::U32,
        }
    }
}

impl Memory {
    /// La zone qui contient tout le bloc, si le chemin rapide est applicable : bloc non vide,
    /// entièrement dans une zone qui autorise l'accès, et aucun point d'observation à prévenir.
    /// Sinon `None` : l'accès se fait octet par octet (mêmes fautes, mêmes observations).
    fn block_region(&mut self, address: u32, len: u32, kind: Kind) -> Option<usize> {
        if len == 0 || !self.watches.is_empty() {
            return None;
        }
        let index = self.find(address, len)?;
        let perms = self.regions[index].spec.perms;
        let allowed = match kind {
            Kind::Read => perms.read,
            Kind::Write => perms.write,
        };
        allowed.then_some(index)
    }
}

impl Bus for Memory {
    fn read8(&mut self, address: u32) -> Result<u8, BusFault> {
        Ok(self.read_access::<1>(address, false)?[0])
    }

    fn read16(&mut self, address: u32) -> Result<u16, BusFault> {
        Ok(u16::from_le_bytes(self.read_access::<2>(address, false)?))
    }

    fn read32(&mut self, address: u32) -> Result<u32, BusFault> {
        Ok(u32::from_le_bytes(self.read_access::<4>(address, false)?))
    }

    fn write8(&mut self, address: u32, value: u8) -> Result<(), BusFault> {
        self.write_access::<1>(address, [value])
    }

    fn write16(&mut self, address: u32, value: u16) -> Result<(), BusFault> {
        self.write_access::<2>(address, value.to_le_bytes())
    }

    fn write32(&mut self, address: u32, value: u32) -> Result<(), BusFault> {
        self.write_access::<4>(address, value.to_le_bytes())
    }

    fn check_exclusive(&mut self, address: u32, write: bool, width: Width) -> Result<(), BusFault> {
        let fault = |kind| BusFault {
            address,
            write,
            width,
            kind,
        };
        let Some(index) = self.find(address, width_bytes(width)) else {
            return Err(fault(FaultKind::Unmapped));
        };
        match self.regions[index].spec.kind {
            RegionKind::Flash => Ok(()),
            RegionKind::Ram | RegionKind::Reserved if self.exclusive_accesses => Ok(()),
            RegionKind::Ram | RegionKind::Reserved => Err(fault(FaultKind::ExclusiveUnsupported)),
        }
    }

    fn read_block(&mut self, address: u32, out: &mut [u8]) -> Result<(), BusFault> {
        match self.block_region(address, out.len() as u32, Kind::Read) {
            Some(index) => {
                let region = &mut self.regions[index];
                let offset = address - region.spec.base;
                region.copy_out(offset, out);
                region.mark_access(offset, out.len() as u32, access::READ);
                Ok(())
            }
            None => {
                for (i, byte) in out.iter_mut().enumerate() {
                    *byte = self.read8(address.wrapping_add(i as u32))?;
                }
                Ok(())
            }
        }
    }

    fn write_block(&mut self, address: u32, data: &[u8]) -> Result<(), BusFault> {
        match self.block_region(address, data.len() as u32, Kind::Write) {
            Some(index) => {
                self.tick += 1;
                let tick = self.tick;
                let region = &mut self.regions[index];
                let offset = address - region.spec.base;
                if region.has_access(offset, data.len() as u32, access::EXECUTE) {
                    self.code_epoch += 1;
                }
                let region = &mut self.regions[index];
                region.copy_in(offset, data);
                region.mark_access(offset, data.len() as u32, access::WRITE);
                region.stamp(offset, data.len() as u32, tick);
                Ok(())
            }
            None => {
                for (i, &byte) in data.iter().enumerate() {
                    self.write8(address.wrapping_add(i as u32), byte)?;
                }
                Ok(())
            }
        }
    }

    fn code_epoch(&self) -> Option<u64> {
        Some(self.code_epoch)
    }

    fn fetch16(&mut self, address: u32) -> Result<u16, BusFault> {
        Ok(u16::from_le_bytes(self.read_access::<2>(address, true)?))
    }
}

/// Valeur little-endian de 1 à 4 octets.
fn le<const N: usize>(bytes: [u8; N]) -> u32 {
    bytes.iter().rev().fold(0, |v, &b| v << 8 | u32::from(b))
}
