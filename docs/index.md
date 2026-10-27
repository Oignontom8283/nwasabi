---
layout: home

# Une application NumWorks ouverte avec `?nwa=…` est envoyée vers l'émulateur, paramètre conservé.
# Le chemin est relatif à la page : il reste juste sous un autre `base` (GitHub Pages…).
head:
  - - script
    - {}
    - "if (new URLSearchParams(location.search).has('nwa')) location.replace('./emulator/' + location.search + location.hash)"

hero:
  name: Nwasabi
  text: Émulateur d'applications NumWorks
  image:
    src: /logo.svg
    alt: Logo de nwasabi
  tagline: Exécutez vos applications externes (.nwa) dans le navigateur, avec un processeur ARM émulé en Rust et WebAssembly.
  actions:
    - theme: brand
      text: Essayer l'émulateur
      link: /emulator/
      target: _self
    - theme: alt
      text: Code source
      link: https://github.com/Oignontom8283/nwasabi
---
