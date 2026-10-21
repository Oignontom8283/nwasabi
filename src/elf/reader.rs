//! Lecture bornée d'entiers little-endian dans un tampon d'octets.

use super::error::ElfError;

/// Vue en lecture seule sur les octets d'un fichier ; chaque accès est vérifié.
#[derive(Clone, Copy)]
pub(crate) struct Reader<'a> {
    data: &'a [u8],
}

impl<'a> Reader<'a> {
    pub(crate) fn new(data: &'a [u8]) -> Self {
        Self { data }
    }

    pub(crate) fn len(&self) -> usize {
        self.data.len()
    }

    /// Sous-tranche `[offset, offset + len)`, ou `Truncated(what)` si elle sort du fichier.
    pub(crate) fn slice(
        &self,
        offset: usize,
        len: usize,
        what: &'static str,
    ) -> Result<&'a [u8], ElfError> {
        let end = offset.checked_add(len).ok_or(ElfError::Truncated(what))?;
        self.data.get(offset..end).ok_or(ElfError::Truncated(what))
    }

    pub(crate) fn u8_at(&self, offset: usize, what: &'static str) -> Result<u8, ElfError> {
        Ok(self.slice(offset, 1, what)?[0])
    }

    pub(crate) fn u16_at(&self, offset: usize, what: &'static str) -> Result<u16, ElfError> {
        let b = self.slice(offset, 2, what)?;
        Ok(u16::from_le_bytes([b[0], b[1]]))
    }

    pub(crate) fn u32_at(&self, offset: usize, what: &'static str) -> Result<u32, ElfError> {
        let b = self.slice(offset, 4, what)?;
        Ok(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_little_endian_values() {
        let r = Reader::new(&[0x01, 0x02, 0x03, 0x04, 0x05]);
        assert_eq!(r.u8_at(4, "x").unwrap(), 5);
        assert_eq!(r.u16_at(0, "x").unwrap(), 0x0201);
        assert_eq!(r.u32_at(0, "x").unwrap(), 0x0403_0201);
    }

    #[test]
    fn refuses_out_of_bounds_reads() {
        let r = Reader::new(&[1, 2, 3]);
        assert_eq!(r.u32_at(0, "en-tête"), Err(ElfError::Truncated("en-tête")));
        assert_eq!(r.u8_at(3, "octet"), Err(ElfError::Truncated("octet")));
        // Un déplacement énorme ne doit pas provoquer de dépassement d'entier.
        assert!(r.slice(usize::MAX, 2, "x").is_err());
    }
}
