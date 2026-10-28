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
[Fichiers `.nfd`](../format/nfd.md) ; il se charge aussi par le [serveur](./serveur.md), l'[émulateur web](./emulateur-web.md) (`?flash=`)
et le [protocole de session](../reference/protocole-session.md).

## Un exemple : KhiCAS

Le lanceur de [KhiCAS](./khicas.md) est installé comme n'importe quelle application, dans la zone des applications. Il ne contient presque
rien : au démarrage, il va lire ses données en lecture seule à `0x90260000`, puis le programme à `0x90400000`, dont il lit le point
d'entrée avant de lui passer la main. Ces deux morceaux sont les données auxiliaires.

<svg viewBox="0 0 720 430" role="img" aria-labelledby="carte-titre carte-desc" style="width:100%;max-width:720px;height:auto;display:block;margin:1.5rem auto;font-family:var(--vp-font-family-base);font-size:12px">
  <title id="carte-titre">Carte de la flash avec KhiCAS</title>
  <desc id="carte-desc">Le lanceur, dans la zone des applications du slot A, lit les données en lecture seule à 0x90260000 et le programme à 0x90400000, dans le slot B.</desc>
  <defs>
    <marker id="fleche" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="7" markerHeight="7" orient="auto-start-reverse">
      <path d="M0 0 L10 5 L0 10 z" fill="var(--vp-c-brand-1)"/>
    </marker>
  </defs>
  <g fill="var(--vp-c-text-2)" text-anchor="end">
    <text x="118" y="24">0x90000000</text>
    <text x="118" y="114">0x90170000</text>
    <text x="118" y="184">0x90260000</text>
    <text x="118" y="296">0x90400000</text>
    <text x="118" y="424">0x90800000</text>
  </g>
  <g stroke="var(--vp-c-divider)" stroke-width="1.5">
    <rect x="130" y="14" width="200" height="86" fill="var(--vp-c-bg-soft)"/>
    <rect x="130" y="100" width="200" height="30" fill="var(--vp-c-brand-soft)" stroke="var(--vp-c-brand-1)"/>
    <rect x="130" y="130" width="200" height="40" fill="var(--vp-c-bg-soft)" stroke-dasharray="4 3"/>
    <rect x="130" y="170" width="200" height="62" fill="var(--vp-c-brand-soft)" stroke="var(--vp-c-brand-1)"/>
    <rect x="130" y="232" width="200" height="50" fill="var(--vp-c-bg-soft)" stroke-dasharray="4 3"/>
    <rect x="130" y="282" width="200" height="132" fill="var(--vp-c-brand-soft)" stroke="var(--vp-c-brand-1)"/>
  </g>
  <line x1="20" y1="282" x2="330" y2="282" stroke="var(--vp-c-text-2)" stroke-width="1.5"/>
  <g fill="var(--vp-c-text-1)" text-anchor="middle">
    <text x="230" y="52">Firmware (Epsilon)</text>
    <text x="230" y="70" fill="var(--vp-c-text-2)">slot A</text>
    <text x="230" y="119" font-weight="600">Lanceur khicas.nwa</text>
    <text x="230" y="154" fill="var(--vp-c-text-2)">zone des applications (libre)</text>
    <text x="230" y="198" font-weight="600">Données en lecture seule</text>
    <text x="230" y="215" fill="var(--vp-c-text-2)">khi110a</text>
    <text x="230" y="261" fill="var(--vp-c-text-2)">libre</text>
    <text x="230" y="340" font-weight="600">Programme KhiCAS</text>
    <text x="230" y="357" fill="var(--vp-c-text-2)">khi110ab.tar</text>
    <text x="230" y="384" fill="var(--vp-c-text-2)">slot B</text>
  </g>
  <path d="M332 112 C 392 112, 392 201, 334 201" fill="none" stroke="var(--vp-c-brand-1)" stroke-width="2" marker-end="url(#fleche)"/>
  <path d="M332 120 C 432 120, 432 348, 334 348" fill="none" stroke="var(--vp-c-brand-1)" stroke-width="2" marker-end="url(#fleche)"/>
  <g fill="var(--vp-c-text-1)">
    <text x="452" y="196">lit les données</text>
    <text x="452" y="332">lit le point d'entrée (0x90400200)</text>
    <text x="452" y="349">puis lui passe la main</text>
  </g>
  <text x="130" y="10" fill="var(--vp-c-text-2)" font-size="11">pas à l'échelle</text>
</svg>

## Où placer les données

La flash externe de 8 Mio est coupée en deux **slots** de 4 Mio. Le firmware, puis les applications installées, occupent le
**slot actif** ; les données auxiliaires vont ailleurs, souvent dans l'autre slot. Un morceau ne peut pas recouvrir le firmware ni une
application installée : il est refusé avec l'adresse en cause.

Si les données doivent être dans le slot que l'appareil exécute par défaut, le `.nfd` peut demander l'autre : c'est le champ
`active_slot` (`--active-slot a` avec `flash pack`). Les adresses des slots et des zones d'applications, selon le modèle, sont dans
[Modèles et mémoire](../reference/modeles-memoire.md#les-deux-slots-de-la-flash).

## Ce que peut faire l'application

Les données sont de la flash comme une autre : lisibles, et **exécutables** à leur adresse (un programme entier peut s'y trouver). Une
application peut aussi effacer et reprogrammer la flash qui n'appartient ni au firmware ni aux applications installées, par exemple
pour garder des scripts ou une session : voir les [appels système](../reference/appels-systeme.md#appels-du-noyau-d-epsilon-utilises-par-khicas)
d'effacement de secteur et d'écriture. Ces écritures font partie de l'état de l'appareil, qu'un fichier
[`.nds`](../format/nds.md) enregistre, mais pas du `.nfd`.

## Avec des données qui changent

Le serveur relit le `.nfd` quand il change sur le disque et propose de recharger. L'application est relancée avec les nouvelles
données, comme pour un nouveau `.nwa`.

## Exemple

[KhiCAS](./khicas.md), le logiciel de calcul formel de Bernard Parisse, en est l'exemple le plus connu : un lanceur de quelques
kilo-octets, un programme de plusieurs mégaoctets et ses données, dans deux zones de la flash.
