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
