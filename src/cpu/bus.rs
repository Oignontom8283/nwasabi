/// Taille d'un accès mémoire.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Width {
    U8,
    U16,
    U32,
}

/// Pourquoi un accès mémoire a échoué.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FaultKind {
    /// Aucune mémoire à cette adresse.
    Unmapped,
    /// La mémoire existe mais l'accès (lecture, écriture ou exécution) n'y est pas permis.
    AccessDenied,
}

/// Un accès mémoire qui a échoué, signalé par un [`Bus`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BusFault {
    pub address: u32,
    pub write: bool,
    pub width: Width,
    pub kind: FaultKind,
}

/// La mémoire que le processeur lit et écrit.
///
/// Toutes les méthodes prennent `&mut self` car un accès peut avoir des effets de bord
/// (registre matériel, suivi des accès…). Les valeurs sont en little-endian.
pub trait Bus {
    fn read8(&mut self, address: u32) -> Result<u8, BusFault>;
    fn read16(&mut self, address: u32) -> Result<u16, BusFault>;
    fn read32(&mut self, address: u32) -> Result<u32, BusFault>;
    fn write8(&mut self, address: u32, value: u8) -> Result<(), BusFault>;
    fn write16(&mut self, address: u32, value: u16) -> Result<(), BusFault>;
    fn write32(&mut self, address: u32, value: u32) -> Result<(), BusFault>;

    /// Numéro qui change chaque fois que du code déjà exécuté a pu être modifié. Le processeur
    /// s'en sert pour garder les instructions décodées : tant qu'il ne change pas, elles sont
    /// toujours valables. `None` (par défaut) : le bus ne le suit pas, rien n'est gardé.
    fn code_epoch(&self) -> Option<u64> {
        None
    }

    /// Lit `out.len()` octets consécutifs (accès du programme). Par défaut octet par octet ; un bus
    /// peut le faire en un seul bloc, ce qui accélère les transferts de pixels et de textes.
    fn read_block(&mut self, address: u32, out: &mut [u8]) -> Result<(), BusFault> {
        for (i, byte) in out.iter_mut().enumerate() {
            *byte = self.read8(address.wrapping_add(i as u32))?;
        }
        Ok(())
    }

    /// Écrit des octets consécutifs (accès du programme).
    fn write_block(&mut self, address: u32, data: &[u8]) -> Result<(), BusFault> {
        for (i, &byte) in data.iter().enumerate() {
            self.write8(address.wrapping_add(i as u32), byte)?;
        }
        Ok(())
    }

    /// Lit un demi-mot d'instruction. Par défaut c'est une simple lecture 16 bits ; un bus peut
    /// la redéfinir pour distinguer le chargement d'instructions de la lecture de données.
    fn fetch16(&mut self, address: u32) -> Result<u16, BusFault> {
        self.read16(address)
    }
}
