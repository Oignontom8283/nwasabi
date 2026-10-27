// Lecture des registres de faute du processeur simulé (bloc SCB, voir `src/scb.rs`) : quels bits sont levés et ce qu'ils disent.

export const SCB = { CPUID: 0xe000ed00, ICSR: 0xe000ed04, SHCSR: 0xe000ed24, CFSR: 0xe000ed28, HFSR: 0xe000ed2c, DFSR: 0xe000ed30, MMFAR: 0xe000ed34, BFAR: 0xe000ed38 };

export interface FaultBit {
  /** Nom du bit dans la documentation ARM. */
  name: string;
  bit: number;
  /** Clé de traduction de l'explication (`faultregs.bits.<clé>`). */
  key: string;
}

/** Les bits de CFSR (MMFSR bits 0-7, BFSR 8-15, UFSR 16-31) que nwasabi remplit. */
export const CFSR_BITS: FaultBit[] = [
  { name: "IACCVIOL", bit: 0, key: "iaccviol" },
  { name: "DACCVIOL", bit: 1, key: "daccviol" },
  { name: "MMARVALID", bit: 7, key: "mmarvalid" },
  { name: "IBUSERR", bit: 8, key: "ibuserr" },
  { name: "PRECISERR", bit: 9, key: "preciserr" },
  { name: "BFARVALID", bit: 15, key: "bfarvalid" },
  { name: "UNDEFINSTR", bit: 16, key: "undefinstr" },
  { name: "INVSTATE", bit: 17, key: "invstate" },
  { name: "UNALIGNED", bit: 24, key: "unaligned" },
];
export const HFSR_BITS: FaultBit[] = [{ name: "FORCED", bit: 30, key: "forced" }];
export const DFSR_BITS: FaultBit[] = [{ name: "BKPT", bit: 1, key: "bkpt" }];

/** Les bits levés de `value` parmi `bits`. */
export function setBits(value: number, bits: FaultBit[]): FaultBit[] {
  return bits.filter((b) => ((value >>> b.bit) & 1) === 1);
}

export interface FaultRegisters {
  cfsr: number;
  hfsr: number;
  dfsr: number;
  mmfar: number;
  bfar: number;
}

/** Y a-t-il quelque chose à montrer ? */
export const hasFault = (r: FaultRegisters): boolean => (r.cfsr | r.hfsr | r.dfsr) !== 0;
