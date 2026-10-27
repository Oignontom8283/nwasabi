// Ce que fait chaque instruction, en français, pour l'infobulle du code désassemblé ; et le genre de saut d'une instruction.

const CONDITIONS: Record<string, string> = {
  eq: "égal (Z = 1)",
  ne: "différent (Z = 0)",
  cs: "retenue levée, supérieur ou égal non signé",
  hs: "retenue levée, supérieur ou égal non signé",
  cc: "retenue éteinte, inférieur non signé",
  lo: "retenue éteinte, inférieur non signé",
  mi: "négatif (N = 1)",
  pl: "positif ou nul (N = 0)",
  vs: "débordement (V = 1)",
  vc: "pas de débordement (V = 0)",
  hi: "supérieur non signé",
  ls: "inférieur ou égal non signé",
  ge: "supérieur ou égal signé",
  lt: "inférieur signé",
  gt: "supérieur signé",
  le: "inférieur ou égal signé",
};

const DOCS: Record<string, string> = {
  mov: "copie une valeur (registre ou constante) dans un registre",
  movw: "met une constante de 16 bits dans les 16 bits de poids faible d'un registre",
  movt: "met une constante de 16 bits dans les 16 bits de poids fort d'un registre, sans toucher aux autres",
  mvn: "copie l'inverse (bit à bit) d'une valeur dans un registre",
  add: "additionne deux valeurs",
  addw: "additionne une constante de 12 bits à un registre, sans modifier les indicateurs",
  adc: "additionne deux valeurs et la retenue",
  sub: "soustrait la deuxième valeur à la première",
  subw: "soustrait une constante de 12 bits à un registre, sans modifier les indicateurs",
  sbc: "soustrait deux valeurs et l'emprunt (retenue inversée)",
  rsb: "soustrait dans l'autre sens : deuxième valeur moins première (rsb r0, r1, #0 donne -r1)",
  mul: "multiplie deux registres (32 bits de poids faible du produit)",
  mla: "multiplie deux registres puis ajoute un troisième",
  mls: "multiplie deux registres puis les soustrait à un troisième",
  umull: "multiplie deux entiers non signés, résultat sur 64 bits (deux registres)",
  smull: "multiplie deux entiers signés, résultat sur 64 bits (deux registres)",
  umlal: "multiplie deux entiers non signés et ajoute au résultat de 64 bits",
  smlal: "multiplie deux entiers signés et ajoute au résultat de 64 bits",
  sdiv: "divise deux entiers signés",
  udiv: "divise deux entiers non signés",
  and: "ET logique bit à bit",
  orr: "OU logique bit à bit",
  eor: "OU exclusif bit à bit",
  bic: "met à zéro les bits de la première valeur qui sont à 1 dans la deuxième",
  orn: "OU logique avec l'inverse de la deuxième valeur",
  lsl: "décale vers la gauche (multiplie par une puissance de 2)",
  lsr: "décale vers la droite, en entrant des zéros (divise par une puissance de 2, non signé)",
  asr: "décale vers la droite en gardant le signe (divise par une puissance de 2, signé)",
  ror: "fait tourner les bits vers la droite",
  rrx: "fait tourner d'un bit vers la droite en passant par la retenue",
  cmp: "compare deux valeurs (soustraction dont on ne garde que les indicateurs) ; la condition qui suit s'en sert",
  cmn: "compare avec l'opposé (addition dont on ne garde que les indicateurs)",
  tst: "teste des bits (ET dont on ne garde que les indicateurs)",
  teq: "teste l'égalité de bits (OU exclusif dont on ne garde que les indicateurs)",
  ldr: "lit un mot de 32 bits en mémoire et le met dans un registre",
  ldrb: "lit un octet en mémoire (complété par des zéros)",
  ldrh: "lit un demi-mot de 16 bits en mémoire (complété par des zéros)",
  ldrsb: "lit un octet en mémoire (signe étendu)",
  ldrsh: "lit un demi-mot de 16 bits en mémoire (signe étendu)",
  ldrd: "lit deux mots de 32 bits consécutifs dans deux registres",
  ldm: "lit plusieurs mots consécutifs de la mémoire dans une liste de registres",
  ldmdb: "lit plusieurs mots de la mémoire, en descendant, dans une liste de registres",
  str: "écrit un registre (32 bits) en mémoire",
  strb: "écrit l'octet de poids faible d'un registre en mémoire",
  strh: "écrit les 16 bits de poids faible d'un registre en mémoire",
  strd: "écrit deux registres dans deux mots consécutifs de la mémoire",
  stm: "écrit une liste de registres dans des mots consécutifs de la mémoire",
  stmdb: "écrit une liste de registres en mémoire, en descendant",
  push: "empile des registres (début d'une fonction : sauvegarde de lr et des registres utilisés)",
  pop: "dépile des registres (fin d'une fonction ; si pc est dans la liste, c'est aussi un retour)",
  b: "saute à une autre instruction (« goto »)",
  bl: "appelle une fonction : sauvegarde l'adresse de retour dans lr puis saute",
  bx: "saute à l'adresse d'un registre ; bx lr retourne à l'appelant",
  blx: "appelle la fonction dont l'adresse est dans un registre",
  cbz: "saute si un registre vaut zéro",
  cbnz: "saute si un registre ne vaut pas zéro",
  tbb: "saute par une table d'octets (un switch compilé)",
  tbh: "saute par une table de demi-mots (un switch compilé)",
  it: "rend conditionnelles les instructions qui suivent (jusqu'à 4)",
  nop: "ne fait rien",
  wfi: "attend une interruption",
  wfe: "attend un événement",
  svc: "appel système : demande un service au noyau (écran, clavier, temps…)",
  bkpt: "point d'arrêt matériel : arrête l'exécution pour le débogueur",
  udf: "instruction indéfinie : provoque une faute",
  mrs: "lit un registre spécial du processeur (état, piles, masques)",
  msr: "écrit un registre spécial du processeur",
  cpsid: "masque les interruptions",
  cpsie: "autorise les interruptions",
  dmb: "barrière mémoire : ordonne les accès à la mémoire",
  clrex: "efface le moniteur d'accès exclusif",
  ldrex: "lecture exclusive (début d'une opération atomique)",
  ldrexb: "lecture exclusive d'un octet",
  ldrexh: "lecture exclusive d'un demi-mot",
  strex: "écriture exclusive (fin d'une opération atomique) ; le registre reçoit 0 si elle a réussi",
  strexb: "écriture exclusive d'un octet",
  strexh: "écriture exclusive d'un demi-mot",
  sxtb: "étend un octet en entier signé de 32 bits",
  sxth: "étend un demi-mot en entier signé de 32 bits",
  uxtb: "étend un octet en entier non signé de 32 bits",
  uxth: "étend un demi-mot en entier non signé de 32 bits",
  clz: "compte les zéros en tête d'un registre",
  rbit: "inverse l'ordre des bits d'un registre",
  rev: "inverse l'ordre des octets d'un mot (change l'endianness)",
  rev16: "inverse l'ordre des octets de chaque demi-mot",
  revsh: "inverse les deux octets de poids faible, signe étendu",
  bfi: "insère un champ de bits d'un registre dans un autre",
  bfc: "met à zéro un champ de bits",
  ubfx: "extrait un champ de bits (non signé)",
  sbfx: "extrait un champ de bits (signé)",
  ssat: "sature un entier signé sur un nombre de bits donné",
  usat: "sature un entier non signé sur un nombre de bits donné",
  adr: "calcule l'adresse d'une donnée proche du code (relative à pc)",
  sel: "choisit octet par octet entre deux registres selon les indicateurs GE",
  vldr: "lit un nombre flottant en mémoire dans un registre flottant",
  vstr: "écrit un registre flottant en mémoire",
  vadd: "additionne deux flottants",
  vsub: "soustrait deux flottants",
  vmul: "multiplie deux flottants",
  vdiv: "divise deux flottants",
  vmov: "copie un flottant (entre registres flottants, ou avec un registre entier)",
  vcmp: "compare deux flottants",
  vcmpe: "compare deux flottants (signale aussi les non-nombres silencieux)",
  vcvt: "convertit entre flottant et entier",
  vcvtr: "convertit un flottant en entier avec l'arrondi du registre FPSCR",
  vsqrt: "racine carrée d'un flottant",
  vabs: "valeur absolue d'un flottant",
  vneg: "opposé d'un flottant",
  vmla: "multiplie deux flottants et ajoute au résultat",
  vmls: "multiplie deux flottants et soustrait au résultat",
  vfma: "multiplication-addition fusionnée de flottants (un seul arrondi)",
  vpush: "empile des registres flottants",
  vpop: "dépile des registres flottants",
  vmrs: "copie le registre d'état flottant (FPSCR) dans un registre ; avec apsr_nzcv, dans les indicateurs",
  vmsr: "écrit le registre d'état flottant (FPSCR)",
};

export interface Described {
  /** Nom de l'instruction sans condition ni suffixe. */
  name: string;
  /** Ce qu'elle fait. */
  text: string;
  /** Condition d'exécution, si l'instruction en a une. */
  condition?: string;
  /** Met à jour les indicateurs (suffixe `s`). */
  flags?: boolean;
}

/** Décrit un mnémonique tel que le désassembleur l'écrit (`addeq`, `movs`, `bne`, `vadd.f32`…). */
export function describe(mnemonic: string): Described | null {
  const word = mnemonic.toLowerCase().split(".")[0] ?? "";
  if (DOCS[word]) return { name: word, text: DOCS[word] };
  const tryBase = (base: string, condition?: string, flags?: boolean): Described | null =>
    DOCS[base] ? { name: base, text: DOCS[base], condition: condition ? CONDITIONS[condition] : undefined, flags } : null;
  // Condition en suffixe (beq, addne, ldreq…), avec ou sans `s` avant (addseq n'existe pas : le `s` vient avant la condition).
  const cond = word.slice(-2);
  if (CONDITIONS[cond]) {
    const rest = word.slice(0, -2);
    const found = tryBase(rest, cond) ?? (rest.endsWith("s") ? tryBase(rest.slice(0, -1), cond, true) : null);
    if (found) return found;
  }
  if (word.endsWith("s")) {
    const found = tryBase(word.slice(0, -1), undefined, true);
    if (found) return found;
  }
  // `it`, `ite`, `itt`, `ittee`… : bloc conditionnel.
  if (/^it[te]{0,3}$/.test(word)) return { name: "it", text: DOCS.it as string };
  return null;
}

export type JumpKind = "jump" | "branch" | "call" | "return" | "indirect";

/** Le genre de saut d'une instruction, ou `null` si ce n'en est pas un. */
export function jumpKind(mnemonic: string, operands: string, hasTarget: boolean): JumpKind | null {
  const m = mnemonic.toLowerCase();
  if (m === "bl") return "call";
  if (m === "blx") return hasTarget ? "call" : "indirect";
  if (m === "bx") return /\blr\b/.test(operands) ? "return" : "indirect";
  if ((m === "pop" || m.startsWith("ldm")) && /\bpc\b/.test(operands)) return "return";
  if (m === "tbb" || m === "tbh") return "indirect";
  if (m === "cbz" || m === "cbnz") return "branch";
  if (m === "b") return hasTarget ? "jump" : null;
  if (/^b(eq|ne|cs|cc|hs|lo|mi|pl|vs|vc|hi|ls|ge|lt|gt|le)$/.test(m)) return "branch";
  return null;
}

/** Texte de l'infobulle d'une instruction. */
export function tooltip(mnemonic: string, operands: string, hasTarget: boolean): string {
  const d = describe(mnemonic);
  const kind = jumpKind(mnemonic, operands, hasTarget);
  if (!d) return mnemonic === ".word" || mnemonic === ".hword" ? "Donnée qui ne se décode pas comme une instruction." : "";
  const parts = [`${d.name} — ${d.text}`];
  if (d.condition) parts.push(`Seulement si : ${d.condition}.`);
  if (d.flags) parts.push("Met à jour les indicateurs (N, Z, C, V).");
  if (kind === "branch") parts.push("Saut conditionnel : suit la cible si la condition est vraie, sinon passe à l'instruction suivante.");
  return parts.join("\n");
}
