# Applications de test en C

De minuscules applications `.nwa`, écrites en C, qui ne font rien d'utile : elles servent à vérifier
qu'un **vrai fichier produit par un compilateur** traverse toute la chaîne de l'émulateur (lecture,
édition de liens, exécution) et que les instructions que GCC choisit donnent les bons résultats.
Elles sont lancées par `tests/emulator_apps.rs`.

| Source | Ce qu'elle vérifie |
|---|---|
| `src/integer.c` | division, multiplications longues, champs de bits, rotations, `rev`, `clz`, table de branchement |
| `src/float.c` | simple précision : `vadd`, `vmul`, `vdiv`, `vsqrt`, `vfma`, conversions, comparaisons non ordonnées |
| `src/keys.c` | colore l'écran selon les touches enfoncées (flèches, OK) : sert aux essais de bout en bout du mapping des touches |
| `src/exclusive_flash.c` | `ldrex` en flash : accepté |
| `src/exclusive_ram.c` | `ldrex` sur la pile : BusFault, comme sur la calculatrice |
| `src/debugme.c` | compilée en `-O0 -g` : fonctions, paramètres, locales, structure, tableau, énumération, récursion, bloc imbriqué ; sert aux tests du débogueur du code source (`tests/debugger.rs`) |
| `src/aux_flash.c` | lit des données auxiliaires en flash (une table, un programme) et en exécute le code ; `tests/apps_flash.rs` les fournit par un `.nfd` |
| `src/flash_rw.c` | efface et programme la flash comme KhiCAS (appels système 30 et 32), vérifie que le firmware est protégé |
| `src/modifiers.c` | lit et écrit l'état de shift et d'alpha (appels système 26 et 28) |

Chaque application se vérifie elle-même (macro `CHECK`) et rend `0` si tout va bien, sinon le numéro de
ligne de la première vérification ratée.

## Les fichiers des autres formats

`bin/` ne contient pas que des `.nwa` : `build.sh` fabrique aussi, avec `nwasabi-cli`, un fichier de chaque format de Nwasabi, que les
tests chargent (`tests/fixtures.rs`, `tests/apps_flash.rs`). Ils sont versionnés comme les `.nwa`.

| Fichier | Format | Ce qu'il sert à vérifier |
|---|---|---|
| `aux_flash.nfd` | données auxiliaires en flash | `aux_flash.nwa` y lit une table et exécute du code |
| `aux_flash.nds` | état | une exécution commencée, reprise avec ses données auxiliaires |
| `keys.nds` | état | reprendre donne exactement la même chose que l'exécution continue |
| `keys.nrp`, `flash_rw.nrp`, `debugme.nrp` | enregistrement | rejouer redonne les mêmes arrêts, instructions et écrans |
| `files.nfs` | fichiers de la calculatrice | chargement, lecture, conservation dans un état |

Les mappings de touches (`.nmk`) ont leur propre dossier, `tests/nmk/`, avec le résultat attendu de chacun. `nwasabi-cli check bin/*` les
contrôle tous (build.sh le fait à la fin).

## Les fichiers `.nwa` sont versionnés

Les binaires de `bin/` sont dans le dépôt : **il n'est pas nécessaire de les recompiler** pour lancer
les tests. On ne les refait qu'en modifiant une source.

```sh
just build-test-apps
```

Cela demande d'installer soi-même :

- un compilateur C pour Cortex-M7, `arm-none-eabi-gcc` (variable `CC` pour en utiliser un autre) ;
- [`nwlink`](https://www.npmjs.com/package/nwlink), que `npx` télécharge au besoin (variable `NWLINK`
  pour pointer vers une installation locale).

Les sources sont compilées en `-O2` (la première ligne d'une source peut ajouter ses options : `// CFLAGS: -O0 -g`), sans bibliothèque standard : une application de test ne doit pas
dépendre d'une fonction extérieure (pas de double précision, pas de division 64 bits).

## Ajouter une application

1. Créer `src/nom.c` (inclure `app.h`, déclarer `TEST_APP("Nom");`, utiliser `CHECK`).
2. `just build-test-apps`.
3. Ajouter un test dans `tests/emulator_apps.rs` qui lance `run("nom")`.
