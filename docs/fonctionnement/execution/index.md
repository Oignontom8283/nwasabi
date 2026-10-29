---
title: "L'exécution coopérative"
description: "Emulator::run, les raisons d'arrêt, le budget d'instructions, les points d'arrêt, l'exécution pas à pas, le profil d'exécution, l'installation d'une application et le lancement."
---

# L'exécution coopérative

`Emulator` réunit un appareil (`DeviceImage` : mémoire, profil), un processeur (`Cpu`), le noyau simulé (`Kernel`) et le système de fichiers (`Storage`). Il ne dort **jamais** et ne lit **aucune horloge** : il avance
l'application **jusqu'à un point où l'hôte doit intervenir**, et dit pourquoi. C'est à l'hôte de faire passer le temps (`Kernel::advance_time`) et de donner les touches.

## Mise en route

1. **`Emulator::with_profile(profil, config)`** : fabrique l'appareil, le noyau et le système de fichiers ; **`new(modèle)`** utilise le profil par défaut du modèle.
2. **`install_nwa(nwa, données_externes)`** : lie le `.nwa` pour la zone de flash libre (`app_layout`), refuse qu'il recouvre un segment de flash déjà placé, l'installe, protège le firmware et les applications (`kernel.flash_protected_end`),
   garde les octets (`AppSource`) pour pouvoir le relier si l'appareil change (profil, slot), puis appelle `launch`.
3. **`install_flash(segments)`** : place des octets en flash à des adresses fixes ([`.nfd`](../../reference/formats/nfd.md)) ; ils ne peuvent pas recouvrir le firmware ni une application installée.
4. **`launch()`** : (re)démarre : `clear_access`, `scb::reset`, attentes oubliées, `Cpu::new()`, `sp` = pile du processus du profil, `lr` = stub de fin `| 1`, `pc` = `_start`, tas fixé sur la région du tas de l'application.
   Refuse en mode examen (`EmulatorError::ExamMode`).

## `Emulator::run(max_instructions) -> RunStop`

`run` et `step` sont les deux points d'entrée. `RunStop` :

| `RunStop` | Signification |
|---|---|
| `Frame` | l'image est prête : l'hôte l'affiche |
| `Sleep { ms }` | l'application dort ; le temps virtuel a déjà avancé |
| `WaitEvent { remaining_ms }` | l'application attend une touche ; l'hôte doit faire passer du temps ou appuyer avant de relancer |
| `Exited(code)` | `main` est revenue |
| `Reset` | l'application a demandé un redémarrage |
| `HomePressed` | l'utilisateur a quitté l'application |
| `Budget` | `max_instructions` est épuisé |
| `Watchpoint(WatchHit)` | un point d'observation « avec arrêt » a été touché ; l'arrêt est juste **après** l'accès |
| `Breakpoint { pc }` | l'exécution est arrivée sur une instruction où l'hôte a posé un point d'arrêt (**pas encore exécutée**) |
| `Fault(Fault)` | l'application s'est arrêtée sur une faute (`Bus`, `Unimplemented`, `UsageFault`, `Breakpoint`, `WaitForInterrupt`, `UnknownSvc`) |

## Boucle interne : `run_inner`

```text
si touche Home demandée : HomePressed
pas_à_pas = (un point d'observation existe) ou (des points d'arrêt existent et le débogage n'est pas suspendu)
tant qu'il reste du budget :
    tranche = 1 si pas_à_pas ; sinon une tranche de profil (≈ 509 ± quelques unités) si le profil est lancé ; sinon tout le reste
    si pas_à_pas :
        si pc porte un point d'arrêt et qu'on n'y repart pas : s'arrêter AVANT, RunStop::Breakpoint
        mémoire.set_watch_pc(pc)
    stop = cpu.run(mémoire, noyau, tranche)
    échantillon de profil si Stop::Budget
    si un point d'observation avec arrêt a été touché : RunStop::Watchpoint
    si stop != Budget : sortir
convertir le dernier Stop en RunStop (noyau.take_stop() pour Requested)
```

Points à retenir :

- **Pas de points d'arrêt, pas d'observation : exécution en bloc**. `Cpu::run` reçoit tout le budget, et le cache d'instructions fait le reste. Dès qu'un point existe, l'émulation passe **instruction par instruction** : plus lente, mais exacte.
- **`resume_from`** : le point d'arrêt où l'on vient de s'arrêter ne s'applique pas deux fois de suite à la même instruction ; `stopped_at_breakpoint` et `resume_point`/`set_resume_point` le sauvegardent quand le débogueur exécute du code
  pour lui-même (appel de fonction d'une expression : [Expressions](../debogage/expressions.md)).
- **Points d'arrêt conditionnels** : `Condition { register, compare, value }` avec `Compare` : `==`, `!=`, `<`, `<=`, `>`, `>=` (non signés) et `&` (au moins un bit en commun). Le registre est `r0`–`r12`, `sp`, `lr` ou `pc`.
- **`suspend_debugging`** : pendant un rejeu de l'historique, points d'arrêt et d'observation sont ignorés sans être retirés.
- **Une tranche de profil de longueur variable** : `PROFILE_SLICE (509) + (cycles >> 3) % 31`. Une longueur fixe retomberait toujours au même endroit d'une boucle et fausserait l'échantillonnage.

## Le profil d'exécution

`start_profile` active un compteur par adresse : à la fin de chaque tranche, l'adresse `pc & !1` gagne un échantillon. `profile_samples` / `stop_profile` rendent `(adresse, nombre)`. La session les regroupe par fonction (adresses sans nom par blocs de 256 octets) et par
adresse. Une application qui attend l'écran ou le clavier exécute trop peu d'instructions pour être profilée.

## Pas à pas

`step()` exécute **une** instruction (`resume_from = pc` puis `run(1)`) **même si un point d'arrêt y est posé**, et rend son `RunStop`. Les pas « par-dessus » et « sortir », ainsi que les pas de source, sont réalisés **par la session** à partir de `step` et des points d'arrêt temporaires, pas par
l'émulateur : voir [Session](../debogage/session.md).

## Utilisation de la mémoire

`usage()` mesure le tas (plage réservée, limite `brk`, plus haute adresse écrite) et la pile (plage réservée, plus basse adresse écrite, `sp` actuel) d'après les indicateurs d'accès de la mémoire, au grain de 32 octets.

## Fautes

À l'arrêt sur une faute, `Emulator` appelle `scb::record` ([Appareil](../memoire/appareil.md#registres-de-faute)) avec l'adresse de l'instruction fautive (`cpu.fault_pc`, ou celle du `svc` pour une faute causée par un pointeur
d'appel système).
