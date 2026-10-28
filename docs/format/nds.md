# Fichiers `.nds` — Nwasabi Dump State

Un fichier **NDS** est une **photographie d'une émulation en pause** : application, mémoire, fichiers,
registres du processeur, écran, clavier, temps. En le chargeant, l'émulation reprend **exactement** où
elle s'était arrêtée : les deux exécutions suivantes (l'originale et la reprise) donnent les mêmes
écrans, les mêmes registres, instruction par instruction.

C'est du JSON, extension `.nds`, contenus binaires encodés en base64. Les parties sont **facultatives** : on
choisit ce que l'on enregistre.

## Parties

| Partie | Contenu | Utile pour |
|---|---|---|
| `device` | **Toujours.** Modèle, disposition du firmware, réglages du profil modifiés (nom, versions, FCC, numéro de série…). | reconstruire l'appareil |
| `app` | Le fichier `.nwa` et ses données externes. | relancer l'application ; **nécessaire en pratique** : sans lui, il faut que l'appareil qui charge le dump ait déjà la même application |
| `memory` | Les pages de mémoire (4 Kio) qui **diffèrent d'un appareil neuf** avec la même application, et les indicateurs d'accès (ce que le programme a lu, écrit, exécuté : marques de la pile et du tas). | l'état des variables, de la pile, du tas |
| `fs` | Les fichiers de la calculatrice, au format [`.nfs`](./nfs.md). | scripts et données de l'application |
| `cpu`, `kernel` | Registres (r0–r12, SP, LR, PC, xPSR, s0–s31, FPSCR), modes, cycles ; écran, clavier (touches tenues, événements en attente, répétition), temps, aléa, tas, attente en cours, batterie, luminosité. `cpu` et `kernel` vont ensemble : c'est la case « état d'exécution ». | reprendre à l'instruction près |

Enregistrer seulement ce qui diffère d'un appareil neuf garde le fichier petit : une application qui n'a
touché que quelques pages de RAM donne quelques ko, pas les 256 Kio de la SRAM.

## Exemple (abrégé)

```json
{
  "nds": 1,
  "created_by": "nwasabi 0.1.0",
  "device": {
    "model": "n0110", "firmware": "24",
    "software_version": "24.11.0", "expected_userland_version": "24.11.0",
    "commit_hash": "a58aaa0", "security_level": 0, "active_slot": "a",
    "fcc_id": "…", "pcb_version": 0, "serial_number": "…", "device_name": "", "exam_mode": false
  },
  "app": { "name": "Keys", "file_name": "keys.nwa", "nwa": "…", "data": "…" },
  "memory": {
    "pages": [ { "address": 537124864, "content": "…" } ],
    "access": [ { "region": "ram", "runs": [ { "line": 120, "flags": "…" } ] } ]
  },
  "fs": { "nfs": 1, "files": [] },
  "cpu": { "r": [0, "…"], "sp": 0, "lr": 0, "pc": 0, "xpsr": 0, "s": ["…"], "fpscr": 0,
           "cycles": 123456, "msp": 0, "psp": 0, "control": 0, "handler_mode": false,
           "vtor": 0, "primask": false, "faultmask": false, "basepri": 0, "exclusive": null },
  "kernel": { "config": {}, "status": {}, "display": { "encoding": "rle16", "pixels": "…" },
              "keyboard": {}, "frames": 3, "extra_us": 0, "…": "…" }
}
```

## Détails

- **`app`** : `name` est le nom que l'application déclare ; `file_name` (facultatif, ajouté par les hôtes qui le connaissent, comme l'émulateur web) est le nom du fichier `.nwa` d'origine ; il n'est pas lu par le cœur. `flash` (facultatif) est le document [`.nfd`](./nfd.md) des octets placés en flash à côté de l'application, rechargés avec elle.
- **`memory.pages`** : `address` est un multiple de 4096 ; `content` couvre une ou plusieurs pages
  consécutives. L'« appareil neuf » de référence est construit comme à la restauration : modèle,
  firmware, réglages de `device`, puis le `.nwa` de la partie `app` **si elle est présente**.
- **`memory.access`** : par zone, des suites `{ line, flags }` d'indicateurs (un octet par ligne de 32
  octets : 1 lu, 2 écrit, 4 exécuté). Les lignes non listées valent 0.
- **Les fichiers ne sont jamais dans `memory`** : le tampon des fichiers est exclu de la comparaison,
  il a sa partie `fs`.
- **`kernel.display`** : l'écran 320×240 en RGB565, `encoding` vaut `raw16` (2 octets par pixel,
  little-endian) ou `rle16` (suites : nombre de répétitions sur 2 octets puis pixel sur 2 octets), le
  plus court des deux.
- Les grands entiers (`state` du clavier : 64 bits) sont écrits en texte pour ne pas perdre de précision.
- Le document est **entièrement vérifié avant** de remplacer quoi que ce soit : s'il est invalide,
  l'appareil actuel n'est pas touché.

## Ce que la restauration garde de l'appareil actuel

Une partie absente du fichier n'est pas effacée : si le dump n'a pas de `app`, l'application de
l'appareil actuel est gardée ; s'il n'a pas de `fs`, ses fichiers sont gardés. Sans `cpu`/`kernel`,
l'application **redémarre** (le dump ne contient alors que les données).

## Limites

- L'état est enregistré **entre deux instructions, simulation en pause** : l'hôte doit arrêter
  l'exécution avant d'enregistrer ou de restaurer.
- Un dump suppose le **même nwasabi** : le format décrit l'état interne, il n'est pas garanti d'une
  version à l'autre du cœur (le champ `nds` donne la version du format, `created_by` celle du cœur).
- Les points d'observation de la mémoire et l'historique des modifications ne sont pas enregistrés.

## Charger un `.nds`

| Où | Comment |
|---|---|
| Émulateur web | `…/emulator/?nds=https://exemple.org/etat.nds` (CORS) ; bouton « Charger un état » de la barre du haut ; l'onglet « Mémoire » enregistre (cases à cocher : application, mémoire, fichiers, état d'exécution ; tout est coché par défaut). Enregistrer et modifier la mémoire exigent la pause. |
| Serveur `nwasabi` | `nwasabi --nds etat.nds` : le fichier est vérifié au démarrage. |
| `nwasabi-cli` | `run --nds etat.nds` reprend (l'application est facultative) ; `run jeu.nwa … --save-nds etat.nds` enregistre après l'exécution. |
| Session JSON | `{"cmd":"dump","parts":["app","memory","fs","state"]}` rend `{"nds": …}` ; `{"cmd":"restore","nds":…}` remplace l'appareil. |
