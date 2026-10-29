---
title: "Le noyau simulé"
description: "Ce que Nwasabi met à la place d'Epsilon : le trait Hooks, les raisons d'arrêt, la configuration, les horloges virtuelle et hôte, le temps et l'attente d'événements."
---

# Le noyau simulé

::: tip En bref
Le noyau est le **guichet** auquel l'application s'adresse par `svc`. Il rend l'écran, le clavier, le temps et les fichiers. Quand il doit attendre, il ne bloque pas : il **s'arrête** et laisse l'hôte décider.
:::


`Kernel` (`src/kernel/`) répond à la place d'Epsilon quand l'application exécute `svc`. Il n'utilise **aucun code d'Epsilon** : il reproduit le comportement observable aux frontières
(valeurs rendues, écran, clavier, temps). Il implémente `cpu::Hooks` : le processeur lui donne chaque `svc #N`, et il rend `Handled`, `HandledAndStop` ou, jamais, `Raise`.

```mermaid
flowchart LR
  K(("Kernel"))
  K --> display["display<br/>écran 320×240 RGB565"]
  K --> keyboard["keyboard<br/>touches, file d'événements,<br/>shift/alpha, répétition"]
  K --> status["status : DeviceStatus<br/>batterie, USB, luminosité,<br/>FCC, série, autorisation…"]
  K --> config["config : KernelConfig<br/>horloge, vitesse, strict_svc,<br/>sortie standard, graine aléatoire"]
  K --> stats["stdout, frames,<br/>svc_counts[256], svc_log"]
  K --> heap["heap (début, fin), brk"]
  K --> prot["flash_protected_end<br/>ce que l'application ne peut pas écrire"]
  K --> st["stop : Option&lt;KernelStop&gt;<br/>wait : Option&lt;EventWait&gt;"]
```

## Pourquoi le noyau s'arrête

Un appel qui doit rendre la main à l'hôte pose un `KernelStop` et rend `HandledAndStop` ; `Cpu::run` s'arrête alors sur `Stop::Requested`, et l'émulateur lit la raison avec
`take_stop()`.

| `KernelStop` | Déclencheur | Ce que fait l'hôte |
|---|---|---|
| `Frame` | `eadk_display_wait_for_vblank` (svc 21) | affiche l'écran |
| `Sleep { ms }` | `msleep`, `usleep` (49, 50) | fait passer `ms` millisecondes de temps réel |
| `WaitEvent { remaining_ms }` | `eadk_event_get` (23) sans événement ; `remaining_ms` : délai restant (`None` : sans fin) | attend une touche ou la fin du délai |
| `Exited(code)` | retour de `main` (svc privé `0x89`) | l'application est finie |
| `Reset` | svc 46 | redémarrage demandé |
| `HomePressed` | touche `Home` | l'utilisateur quitte l'application |
| `BusFault` | un pointeur passé par l'application sort de la mémoire | faute, rapportée sur l'instruction `svc` |
| `UnknownSvc(n)` | numéro inconnu **avec** `strict_svc` | faute |

## Rejouer l'appel plutôt que bloquer

Une attente (`event_get` sans événement, `usleep` avec l'horloge hôte) **ne bloque pas** : l'émulateur n'a pas de fil à endormir. Le noyau fait deux choses : il **recule `pc` de 2 octets** (l'instruction `svc` fait 2 octets) et il **s'arrête**. À la reprise, le processeur retombe sur le même `svc` et **réévalue la condition**.

```mermaid
sequenceDiagram
  participant A as Application
  participant K as Noyau
  participant H as Hôte
  A->>K: svc 23 (eadk_event_get, délai 500 ms)
  K->>K: aucun événement : noter le début de l'attente<br/>pc ← pc − 2
  K-->>H: KernelStop::WaitEvent { remaining_ms: 500 }
  H->>K: fait passer 200 ms (advance), ou appuie sur une touche
  H->>A: relance run
  A->>K: svc 23 (la même instruction, rejouée)
  alt un événement est arrivé
    K-->>A: r0 = événement, délai restant écrit
  else le délai est écoulé
    K-->>A: r0 = EVENT_NONE
  else rien encore
    K-->>H: WaitEvent { remaining_ms: 300 }
  end
```

C'est ce qui rend l'attente **interruptible**, **déterministe** et **sans fil** : tout l'état de l'attente tient dans `EventWait { started_us, timeout_ms }` (ou `sleep_until_us`), et se sauvegarde dans un `.nds`.

## Le temps

Il y a deux horloges (`ClockMode`) :

| Mode | `micros(cycles)` | Usage |
|---|---|---|
| `Virtual` (défaut) | `cycles / cycles_par_µs + extra_us` : le temps avance avec les instructions exécutées **et** par les attentes de l'application | tests, ligne de commande, enregistrements : même exécution, même résultat, sans jamais attendre |
| `Host` | `extra_us` seulement : le temps n'avance que par `advance_time(us)`, que l'hôte appelle avec le temps réellement écoulé | la page web : en pause, l'application ne voit pas le temps passer |

```mermaid
flowchart LR
  subgraph V["Horloge virtuelle (défaut)"]
    v1["cycles exécutés"] --> v2["÷ MHz du modèle"] --> v3["+ temps des attentes"] --> v4(["micros()"])
  end
  subgraph H["Horloge hôte (page web)"]
    h1["advance(µs)<br/>donné par l'hôte<br/>= temps réellement écoulé"] --> h4(["micros()"])
  end
```

`cycles_per_us` est la fréquence en MHz du modèle (192 sur N0110 et N0115, 540 sur N0120), ou la fréquence choisie (`CpuSpeed::Custom`, `DEFAULT_CUSTOM_MHZ = 192`, au plus 100 000), ou celle du modèle pour
`CpuSpeed::Max` — qui n'impose aucune **limite** à l'hôte (`limit_mhz()` rend `None`) mais garde le temps virtuel à la fréquence du modèle. Une image dure `FRAME_US = 16 667 µs` (60 Hz) : avec l'horloge virtuelle,
`wait_for_vblank` saute à la prochaine multiple ; avec l'horloge hôte, c'est l'hôte qui règle la cadence.

## Réglages

`KernelConfig` : `clock`, `speed`, `custom_mhz`, `strict_svc` (un appel inconnu arrête l'application ; sinon il est compté dans `unknown_svcs` et ignoré, comme sur l'appareil), `stdout_to_screen` (le texte
du `write` de la bibliothèque C est aussi dessiné en haut de l'écran) et `random_seed` (xorshift32 ; même graine, même suite).

`DeviceStatus` : niveau et tension de la batterie, charge, USB, luminosité (4 bits : lue et écrite par pas de 16), niveau d'autorisation (`clearance_level`), drapeaux de compilation, CRC du bootloader, type du dernier
redémarrage, version du PCB, numéro de série, adresse de la chaîne FCC, fréquence du processeur. Les valeurs de départ viennent du profil du firmware.

## Instrumentation

- `svc_counts[n]` compte chaque numéro (statistiques de la page) ;
- `svc_log` (démarré à la demande) garde les **2000 derniers** appels : numéro, adresse du `svc`, `r0`–`r3` à l'entrée, `r0` au retour, image en cours (`SvcLog::CAPACITY`) ;
- `stdout` garde au plus 1 Mio (`MAX_STDOUT`).

## Enregistrer l'état

`state.rs` implémente `to_state` et `restore_state` : écran, clavier (touches tenues, événements en attente, répétition), temps, compteurs, aléa, tas, attente en cours, réglages. Seul `stop` n'y est pas : il est toujours
consommé avant que l'hôte reprenne la main. L'écran est enregistré en `raw16` (deux octets par pixel) ou en `rle16` (nombre de répétitions sur deux octets puis le pixel), le plus court des deux. En cas d'erreur de
lecture, le noyau n'est pas modifié.

## Pages

- [Les appels système](./appels-systeme.md)
- [Écran et police](./ecran-et-police.md)
- [Clavier et événements](./clavier.md)
- [Le système de fichiers](./fichiers.md)

## Dans le code

Le module est [`src/kernel/`](code:src/kernel/) :

| Fichier | Rôle | À regarder |
|---|---|---|
| [`mod.rs`](code:src/kernel/mod.rs) | l'état, la configuration, le temps | [`Kernel`](code:src/kernel/mod.rs#Kernel), [`KernelStop`](code:src/kernel/mod.rs#KernelStop), [`KernelConfig`](code:src/kernel/mod.rs#KernelConfig), [`ClockMode`](code:src/kernel/mod.rs#ClockMode), [`micros`](code:src/kernel/mod.rs#micros), [`next_event`](code:src/kernel/mod.rs#next_event) |
| [`svc.rs`](code:src/kernel/svc.rs) | le traitement de chaque appel | [`handle`](code:src/kernel/svc.rs#handle), [`on_svc`](code:src/kernel/svc.rs#on_svc) |
| [`display.rs`](code:src/kernel/display.rs) | l'écran | [`Display`](code:src/kernel/display.rs#Display), [`Rect`](code:src/kernel/display.rs#Rect), [`draw_text`](code:src/kernel/display.rs#draw_text) |
| [`keyboard.rs`](code:src/kernel/keyboard.rs) | touches et événements | [`Keyboard`](code:src/kernel/keyboard.rs#Keyboard), [`press`](code:src/kernel/keyboard.rs#press), [`Key`](code:src/kernel/keyboard.rs#Key) |
| [`event_text.rs`](code:src/kernel/event_text.rs) | la table des textes d'événements | [`event_text`](code:src/kernel/event_text.rs#event_text) |
| [`font/`](code:src/kernel/font/) | la police | [`draw_glyph`](code:src/kernel/font/mod.rs#draw_glyph) |
| [`state.rs`](code:src/kernel/state.rs) | enregistrer et restaurer l'état | [`to_state`](code:src/kernel/state.rs#to_state), [`restore_state`](code:src/kernel/state.rs#restore_state) |
| [`tests.rs`](code:src/kernel/tests.rs) | tests unitaires | |

## Pourquoi ainsi

- **Le noyau implémente `Hooks`** ([`impl Hooks for Kernel`](<code:src/kernel/svc.rs#~impl Hooks for Kernel>)) : le processeur ne sait pas qu'il y a un noyau. On pourrait brancher un autre `Hooks` (par exemple pour tester le processeur seul, comme le font les vecteurs QEMU).
- **Un `svc` n'est jamais bloquant** : une attente pose un [`KernelStop`](code:src/kernel/mod.rs#KernelStop) et **recule `pc`**. Un thread endormi aurait exigé un second fil ou une machine à états extérieure ; avec le rejeu, l'état de l'attente tient dans quelques champs qu'on **sauvegarde** (`.nds`) et qu'on **copie** (points de reprise).
- **Le noyau n'écrit pas dans la mémoire de l'appareil par ses propres chemins** : il utilise le `Bus` qu'on lui passe, donc ses accès sont suivis et observables comme ceux de l'application.
- **Aucune dépendance au matériel** : l'écran est un tableau de `u16`, pas une surface ; l'hôte le lit et le dessine.

## Pour modifier

- **Ajouter un appel système** : voir [Implémentation des appels système](./appels-systeme.md#pour-modifier).
- **Ajouter un état au noyau** : le champ doit aussi être dans [`to_state`](code:src/kernel/state.rs#to_state) et [`restore_state`](code:src/kernel/state.rs#restore_state), sinon un `.nds` ou un recul dans l'historique perdrait cette information. Le noyau est `Clone` (c'est ainsi que l'historique le copie) : un champ non clonable est un problème de conception.
- **Changer le temps** : tout passe par [`micros`](code:src/kernel/mod.rs#micros) ; ne jamais lire d'horloge réelle dans le cœur.
- **Tests** : [`kernel/tests.rs`](code:src/kernel/tests.rs), [`session.rs`](code:tests/session.rs), [`emulator_apps.rs`](code:tests/emulator_apps.rs).
