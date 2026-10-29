#!/bin/sh
# Compile les applications de test (tests/apps/src/*.c) en tests/apps/bin/*.nwa.
#
# Outils nécessaires (à installer soi-même, ce script n'est lancé que rarement) :
#   - un compilateur C pour Cortex-M7 : arm-none-eabi-gcc (variable CC pour en changer) ;
#   - nwlink (paquet npm) : `npx --yes -- nwlink@1.0.0` par défaut (variable NWLINK pour en changer).
set -eu
cd "$(dirname "$0")"

CC=${CC:-arm-none-eabi-gcc}
NWLINK=${NWLINK:-npx --yes -- nwlink@1.0.0}
OUT=${OUT:-build}

mkdir -p "$OUT" bin
$NWLINK png-icon-o icon.png "$OUT/icon.o"
# `-fdebug-prefix-map` : les chemins de débogage sont relatifs à ce dossier, pas à la machine qui compile.
CFLAGS="$($NWLINK eadk-cflags-device) -O2 -ffreestanding -fno-math-errno -Wall -Wextra -fdebug-prefix-map=$(pwd)=."
LDFLAGS="$($NWLINK eadk-ldflags-device) -nostdlib"

for source in src/*.c; do
    name=$(basename "$source" .c)
    # nwlink rend ses options avec des guillemets (chemin d'inclusion) : eval les relit.
    # Une application peut ajouter ses options sur sa première ligne : `// CFLAGS: -O0 -g` (les dernières l'emportent).
    extra=$(sed -n '1s|^// CFLAGS: ||p' "$source")
    eval "$CC $CFLAGS $extra -c $source -o $OUT/$name.o"
    eval "$CC $CFLAGS $LDFLAGS $OUT/$name.o $OUT/icon.o -o bin/$name.nwa"
    echo "bin/$name.nwa"
done

# ---- fichiers des autres formats --------------------------------------------------------------------------------------
# Les tests (tests/fixtures.rs, tests/apps_flash.rs…) lisent ces fichiers tels qu'ils sont dans bin/ ; on ne les refait qu'ici, avec
# nwasabi-cli (variable NWASABI_CLI pour en utiliser un autre). Ils vont avec les .nwa : bin/aux_flash.nfd, bin/keys.nrp…
CLI=${NWASABI_CLI:-"env NWASABI_NO_WEB=1 cargo run --quiet --manifest-path ../../Cargo.toml --features cli --bin nwasabi-cli --"}

# Données auxiliaires de aux_flash.nwa : un programme (signature « NWAUX », puis `movs r0, #42 ; bx lr`) et une table de 16 mots
# (le mot n vaut 0x1000 + 7n).
printf 'NWAUX\000\000\000\052\040\160\107' > "$OUT/aux_program.bin"
: > "$OUT/aux_table.bin"
i=0
while [ "$i" -lt 16 ]; do
    value=$((0x1000 + 7 * i))
    printf "\\$(printf '%03o' $((value & 255)))\\$(printf '%03o' $(((value >> 8) & 255)))\\000\\000" >> "$OUT/aux_table.bin"
    i=$((i + 1))
done
$CLI flash pack -o bin/aux_flash.nfd --active-slot a "0x90400000=$OUT/aux_program.bin" "0x90260000=$OUT/aux_table.bin"
# Un état en cours d'exécution : l'application et ses données auxiliaires.
$CLI run bin/aux_flash.nwa --flash bin/aux_flash.nfd --instructions 40 --save-nds bin/aux_flash.nds --json > /dev/null

# Enregistrements : une exécution avec une touche, une application qui écrit dans la flash, une application déboguée.
$CLI run bin/keys.nwa --press ok --frames 6 --record bin/keys.nrp --json > /dev/null
$CLI run bin/flash_rw.nwa --record bin/flash_rw.nrp --json > /dev/null
$CLI run bin/debugme.nwa --record bin/debugme.nrp --json > /dev/null
# Un état de l'application keys après quelques images, avec la touche enfoncée.
$CLI run bin/keys.nwa --press ok --frames 3 --save-nds bin/keys.nds --json > /dev/null

# Les fichiers de la calculatrice : un script Python et un fichier de données.
rm -f "$OUT/files.img"
$CLI fs --image "$OUT/files.img" write main.py --text 'print("bonjour")' > /dev/null
$CLI fs --image "$OUT/files.img" write scores.py --text 'best = 42' > /dev/null
$CLI fs --image "$OUT/files.img" export bin/files.nfs > /dev/null

# Un contrôle : tout ce qui est dans bin/ doit être lisible.
$CLI check bin/*
