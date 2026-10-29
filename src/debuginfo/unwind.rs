//! Déroulement de la pile avec les tables d'appel de `.debug_frame` (CFI) : des cadres exacts, sans deviner.

use gimli::{
    BaseAddresses, CfaRule, DebugFrame, EndianSlice, LittleEndian, RegisterRule, UnwindContext,
    UnwindSection,
};

use super::DebugInfo;

/// Un cadre de la pile d'appels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Frame {
    /// Adresse de l'instruction : l'instruction en cours pour le cadre 0, l'adresse de retour pour les autres.
    pub pc: u32,
    /// Les registres r0 à r15 dans ce cadre (ceux que la fonction appelée n'a pas sauvegardés ne sont pas connus : on garde leur valeur).
    pub regs: [u32; 16],
    /// Adresse de la pile à l'entrée de la fonction (la « CFA »), si les tables la donnent.
    pub cfa: Option<u32>,
}

impl DebugInfo {
    /// L'adresse de la pile à l'entrée de la fonction de `pc`, avec les registres `regs` de ce cadre.
    pub fn cfa_at(&self, regs: &[u32; 16], pc: u32, is_return_address: bool) -> Option<u32> {
        self.unwind_row(regs, pc, is_return_address)
            .map(|(cfa, _)| cfa)
    }

    /// Le cadre appelant `regs` (qui exécute `pc`) : ses registres et sa CFA. `None` au sommet de la pile ou sans tables.
    pub fn caller(
        &self,
        regs: &[u32; 16],
        pc: u32,
        is_return_address: bool,
        read: &mut dyn FnMut(u32, usize) -> Option<Vec<u8>>,
    ) -> Option<[u32; 16]> {
        let (cfa, rules) = self.unwind_row(regs, pc, is_return_address)?;
        let mut out = *regs;
        for (reg, rule) in rules {
            let index = usize::from(reg);
            if index >= 16 {
                continue;
            }
            match rule {
                Rule::Undefined => {
                    // `lr` indéfini : on est en haut de la pile.
                    if index == 14 {
                        return None;
                    }
                }
                Rule::Same => {}
                Rule::Offset(n) => {
                    let bytes = read(cfa.wrapping_add(n as u32), 4)?;
                    out[index] = u32::from_le_bytes(bytes.try_into().ok()?);
                }
                Rule::ValOffset(n) => out[index] = cfa.wrapping_add(n as u32),
                Rule::Register(r) => out[index] = *regs.get(usize::from(r))?,
            }
        }
        out[13] = cfa;
        // L'adresse de retour est dans lr ; une fonction sans règle pour lr le garde (feuille).
        out[15] = out[14] & !1;
        Some(out)
    }

    fn unwind_row(
        &self,
        regs: &[u32; 16],
        pc: u32,
        is_return_address: bool,
    ) -> Option<(u32, Vec<(u16, Rule)>)> {
        let data = self.frame_section()?;
        let mut section = DebugFrame::new(data, LittleEndian);
        section.set_address_size(4);
        let section: DebugFrame<EndianSlice<'_, LittleEndian>> = section;
        let bases = BaseAddresses::default();
        let mut ctx = UnwindContext::new();
        // Pour un cadre appelant, `pc` est l'adresse qui suit l'appel : la fonction est celle de l'instruction d'avant.
        let lookup = if is_return_address {
            pc.wrapping_sub(1)
        } else {
            pc
        };
        let row = section
            .unwind_info_for_address(
                &bases,
                &mut ctx,
                u64::from(lookup),
                DebugFrame::cie_from_offset,
            )
            .ok()?;
        let cfa = match row.cfa() {
            CfaRule::RegisterAndOffset { register, offset } => regs
                .get(usize::from(register.0))?
                .wrapping_add(*offset as u32),
            CfaRule::Expression(_) => return None,
        };
        let mut rules = Vec::new();
        for r in 0..16u16 {
            let rule = match row.register(gimli::Register(r)) {
                None => continue,
                Some(RegisterRule::Undefined) => Rule::Undefined,
                Some(RegisterRule::SameValue) => Rule::Same,
                Some(RegisterRule::Offset(n)) => Rule::Offset(n),
                Some(RegisterRule::ValOffset(n)) => Rule::ValOffset(n),
                Some(RegisterRule::Register(other)) => Rule::Register(other.0),
                Some(_) => continue,
            };
            rules.push((r, rule));
        }
        Some((cfa, rules))
    }

    /// La pile d'appels depuis `regs` (cadre 0 : l'instruction en cours), au plus `max` cadres.
    pub fn unwind(
        &self,
        regs: &[u32; 16],
        max: usize,
        read: &mut dyn FnMut(u32, usize) -> Option<Vec<u8>>,
    ) -> Vec<Frame> {
        let mut frames = Vec::new();
        let mut regs = *regs;
        let mut is_return = false;
        while frames.len() < max {
            let pc = regs[15];
            let cfa = self.cfa_at(&regs, pc, is_return);
            frames.push(Frame { pc, regs, cfa });
            let next = match self.caller(&regs, pc, is_return, read) {
                Some(next) => next,
                // Le code que le compilateur n'a pas décrit (les fonctions de `eadk`, écrites à la main) est une feuille :
                // l'adresse de retour est dans lr, la pile n'a pas bougé. Seul le premier cadre peut s'y fier.
                None if frames.len() == 1
                    && self.function_at(pc).is_none()
                    && regs[14] & 1 == 1 =>
                {
                    let mut next = regs;
                    next[15] = regs[14] & !1;
                    next
                }
                None => break,
            };
            // Pas de progression (pile ou adresse identique) : on arrête.
            if next[15] == 0 || (next[13] == regs[13] && next[15] == regs[15]) {
                break;
            }
            regs = next;
            is_return = true;
        }
        frames
    }
}

enum Rule {
    Undefined,
    Same,
    Offset(i64),
    ValOffset(i64),
    Register(u16),
}
