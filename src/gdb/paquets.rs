//! Les petits éléments du protocole GDB : la description des registres donnée au débogueur, la somme de contrôle d'un paquet et
//! l'écriture en hexadécimal.

/// Taille maximale d'un paquet annoncée au débogueur.
pub(super) const PACKET_SIZE: usize = 0x4000;

/// La description des registres donnée au débogueur (profil M d'ARM, numérotation simple 0 à 16).
pub(super) const TARGET_XML: &str = r#"<?xml version="1.0"?>
<!DOCTYPE target SYSTEM "gdb-target.dtd">
<target version="1.0">
  <architecture>arm</architecture>
  <feature name="org.gnu.gdb.arm.m-profile">
    <reg name="r0" bitsize="32" regnum="0"/>
    <reg name="r1" bitsize="32" regnum="1"/>
    <reg name="r2" bitsize="32" regnum="2"/>
    <reg name="r3" bitsize="32" regnum="3"/>
    <reg name="r4" bitsize="32" regnum="4"/>
    <reg name="r5" bitsize="32" regnum="5"/>
    <reg name="r6" bitsize="32" regnum="6"/>
    <reg name="r7" bitsize="32" regnum="7"/>
    <reg name="r8" bitsize="32" regnum="8"/>
    <reg name="r9" bitsize="32" regnum="9"/>
    <reg name="r10" bitsize="32" regnum="10"/>
    <reg name="r11" bitsize="32" regnum="11"/>
    <reg name="r12" bitsize="32" regnum="12"/>
    <reg name="sp" bitsize="32" type="data_ptr" regnum="13"/>
    <reg name="lr" bitsize="32" regnum="14"/>
    <reg name="pc" bitsize="32" type="code_ptr" regnum="15"/>
    <reg name="xpsr" bitsize="32" regnum="16"/>
  </feature>
</target>
"#;

/// Somme de contrôle d'un paquet : la somme des octets modulo 256.
pub(super) fn checksum(data: &[u8]) -> u8 {
    data.iter().fold(0u8, |sum, &b| sum.wrapping_add(b))
}

fn hex_byte(b: u8) -> [u8; 2] {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    [DIGITS[usize::from(b >> 4)], DIGITS[usize::from(b & 15)]]
}

pub(super) fn to_hex(bytes: &[u8]) -> String {
    bytes
        .iter()
        .flat_map(|&b| hex_byte(b))
        .map(char::from)
        .collect()
}

pub(super) fn from_hex(text: &str) -> Option<Vec<u8>> {
    if !text.len().is_multiple_of(2) {
        return None;
    }
    (0..text.len() / 2)
        .map(|i| u8::from_str_radix(text.get(2 * i..2 * i + 2)?, 16).ok())
        .collect()
}

pub(super) fn word_hex(value: u32) -> String {
    to_hex(&value.to_le_bytes())
}
