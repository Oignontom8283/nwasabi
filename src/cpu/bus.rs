/// Taille d'un accès mémoire.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Width {
    U8,
    U16,
    U32,
}

/// Un accès mémoire qui a échoué, signalé par un [`Bus`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BusFault {
    pub address: u32,
    pub write: bool,
    pub width: Width,
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

    /// Lit un demi-mot d'instruction. Par défaut c'est une simple lecture 16 bits ; un bus peut
    /// la redéfinir pour distinguer le chargement d'instructions de la lecture de données.
    fn fetch16(&mut self, address: u32) -> Result<u16, BusFault> {
        self.read16(address)
    }
}
