// Une instruction désassemblée, telle que la commande `disasm` du module la rend.
export interface AsmLine {
  address: number;
  size: number;
  /** Les demi-mots de l'instruction, en hexadécimal. */
  hex: string;
  mnemonic: string;
  operands: string;
  /** Adresse visée par un saut, un appel ou une lecture relative au PC. */
  target?: number;
  target_symbol?: string;
  /** Nom de la fonction qui commence ici. */
  label?: string;
}
