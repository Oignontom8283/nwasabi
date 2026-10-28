# Données auxiliaires en flash

Une application externe tient normalement dans son fichier `.nwa`. Certaines, trop volumineuses ou qui ont besoin de données
préparées à part, lisent à la place des **données auxiliaires en flash** : un programme ou des fichiers que l'on a écrits dans la
flash de la calculatrice, à des **adresses fixes**, avec un outil USB, à côté de l'application. Le `.nwa` n'est alors souvent qu'un
lanceur qui va chercher ces données.

Sans elles, l'application ne trouve rien à l'adresse attendue et s'arrête. Nwasabi les reproduit avec un fichier
[`.nfd`](../format/nfd.md) : une liste de morceaux d'octets, chacun avec son adresse.

## À quoi ça ressemble

Sur une vraie calculatrice, on flashe chaque morceau à son adresse :

```sh
dfu-util -i 0 -a 0 -s 0x90400000 -D programme.tar
dfu-util -i 0 -a 0 -s 0x90260000 -D donnees.bin
```

Avec Nwasabi, on rassemble les mêmes fichiers et leurs adresses dans un `.nfd`, puis on le donne avec l'application :

```sh
nwasabi-cli flash pack -o morceaux.nfd 0x90400000=programme.tar 0x90260000=donnees.bin
nwasabi-cli run lanceur.nwa --flash morceaux.nfd
```

Chaque fichier est un simple fichier binaire : Nwasabi en copie les octets tels quels. Le format `.nfd` est décrit dans
[Fichiers `.nfd`](../format/nfd.md) ; il se charge aussi par le [serveur](../guide/serveur.md), l'[émulateur web](../guide/emulateur-web.md) (`?flash=`)
et le [protocole de session](./protocole-session.md).

## Exemple : KhiCAS

Le lanceur de [KhiCAS](../guide/khicas.md) est installé comme n'importe quelle application, dans la zone des applications. Il ne contient
presque rien : au démarrage, il va lire ses données en lecture seule à `0x90260000`, puis le programme à `0x90400000`, dont il lit le
point d'entrée avant de lui passer la main. Ces deux morceaux sont les données auxiliaires.

<svg viewBox="0 0 800 430" role="img" aria-label="Carte de la flash avec KhiCAS : le lanceur, dans la zone des applications du slot A, lit les données en lecture seule à 0x90260000 et le programme à 0x90400000, dans le slot B." style="width:100%;max-width:800px;height:auto;display:block;margin:1.5rem auto;font-family:var(--vp-font-family-base);font-size:12px">
  <defs>
    <marker id="fleche" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="7" markerHeight="7" orient="auto-start-reverse">
      <path d="M0 0 L10 5 L0 10 z" fill="var(--vp-c-brand-1)"/>
    </marker>
  </defs>

  <!-- les slots : une accolade à gauche de la carte, avec son nom -->
  <g fill="none" stroke-width="3" stroke-linecap="round" stroke-linejoin="round">
    <path d="M94 14 H84 V280 H94" stroke="var(--vp-c-indigo-1)"/>
    <path d="M94 284 H84 V414 H94" stroke="var(--vp-c-green-1)"/>
  </g>
  <g font-size="17" font-weight="700" text-anchor="start">
    <text x="6" y="153" fill="var(--vp-c-indigo-1)">Slot A</text>
    <text x="6" y="353" fill="var(--vp-c-green-1)">Slot B</text>
  </g>

  <!-- les adresses -->
  <g fill="var(--vp-c-text-2)" text-anchor="end">
    <text x="204" y="24">0x90000000</text>
    <text x="204" y="114">0x90170000</text>
    <text x="204" y="184">0x90260000</text>
    <text x="204" y="296">0x90400000</text>
    <text x="204" y="402">0x90795800</text>
    <text x="204" y="426">0x90800000</text>
  </g>

  <!-- la mémoire -->
  <g stroke="var(--vp-c-divider)" stroke-width="1.5">
    <rect x="216" y="14" width="200" height="86" fill="var(--vp-c-bg-soft)"/>
    <rect x="216" y="100" width="200" height="30" fill="var(--vp-c-brand-soft)" stroke="var(--vp-c-brand-1)"/>
    <rect x="216" y="130" width="200" height="40" fill="var(--vp-c-bg-soft)" stroke-dasharray="4 3"/>
    <rect x="216" y="170" width="200" height="62" fill="var(--vp-c-brand-soft)" stroke="var(--vp-c-brand-1)"/>
    <rect x="216" y="232" width="200" height="50" fill="var(--vp-c-bg-soft)" stroke-dasharray="4 3"/>
    <rect x="216" y="282" width="200" height="118" fill="var(--vp-c-brand-soft)" stroke="var(--vp-c-brand-1)"/>
    <rect x="216" y="400" width="200" height="14" fill="var(--vp-c-bg-soft)" stroke-dasharray="4 3"/>
  </g>
  <line x1="98" y1="282" x2="416" y2="282" stroke="var(--vp-c-text-2)" stroke-width="1.5"/>
  <g fill="var(--vp-c-text-1)" text-anchor="middle">
    <text x="316" y="52">Firmware (Epsilon)</text>
    <text x="316" y="70" fill="var(--vp-c-text-2)">slot A</text>
    <text x="316" y="119" font-weight="600">Lanceur khicas.nwa</text>
    <text x="316" y="154" fill="var(--vp-c-text-2)">zone des applications (libre)</text>
    <text x="316" y="198" font-weight="600">Données en lecture seule</text>
    <text x="316" y="215" fill="var(--vp-c-text-2)">khi110a</text>
    <text x="316" y="261" fill="var(--vp-c-text-2)">libre</text>
    <text x="316" y="340" font-weight="600">Programme KhiCAS</text>
    <text x="316" y="357" fill="var(--vp-c-text-2)">khi110ab.tar</text>
    <text x="316" y="378" fill="var(--vp-c-text-2)">3,58 Mio</text>
  </g>

  <!-- ce que fait le lanceur -->
  <g fill="none" stroke="var(--vp-c-brand-1)" stroke-width="2">
    <path d="M418 108 C 476 108, 476 201, 420 201" marker-end="url(#fleche)"/>
    <path d="M418 118 C 520 118, 520 348, 420 348" marker-end="url(#fleche)"/>
  </g>
  <g fill="var(--vp-c-text-1)">
    <text x="506" y="196">lit les données</text>
    <text x="540" y="338">lit le point d'entrée (0x90400200)</text>
    <text x="540" y="355">puis lui passe la main</text>
  </g>
  <text x="216" y="10" fill="var(--vp-c-text-2)" font-size="11">Pas à l'échelle</text>
</svg>

Le lanceur occupe un secteur de 64 Kio, comme toute application. Le firmware est dans le slot A, ce qui laisse le slot B au programme.

## Où placer les données

La flash externe de 8 Mio est coupée en deux **slots** de 4 Mio. Le firmware, puis les applications installées, occupent le
**slot actif** ; les données auxiliaires vont ailleurs, souvent dans l'autre slot. Un morceau ne peut pas recouvrir le firmware ni une
application installée : il est refusé avec l'adresse en cause.

Si les données doivent être dans le slot que l'appareil exécute par défaut, le `.nfd` peut demander l'autre : c'est le champ
`active_slot` (`--active-slot a` avec `flash pack`). Les adresses des slots et des zones d'applications, selon le modèle, sont dans
[Modèles et mémoire](./modeles-memoire.md#les-deux-slots-de-la-flash).

## Ce que peut faire l'application

Les données sont de la flash comme une autre : lisibles, et **exécutables** à leur adresse (un programme entier peut s'y trouver). Une
application peut aussi effacer et reprogrammer la flash qui n'appartient ni au firmware ni aux applications installées, par exemple
pour garder des scripts ou une session : voir les [appels système](./appels-systeme.md#appels-du-noyau-d-epsilon-utilises-par-khicas)
d'effacement de secteur et d'écriture. Ces écritures font partie de l'état de l'appareil, qu'un fichier
[`.nds`](../format/nds.md) enregistre, mais pas du `.nfd`.

## Avec des données qui changent

Le serveur relit le `.nfd` quand il change sur le disque et propose de recharger. L'application est relancée avec les nouvelles
données, comme pour un nouveau `.nwa`.
