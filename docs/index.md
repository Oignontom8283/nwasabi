---
layout: home
title: "Nwasabi"
description: "Émulateur d'applications externes NumWorks (.nwa) en Rust et WebAssembly : processeur Cortex-M7 émulé, débogueur (GDB, C, C++, Rust), serveur local et bibliothèque JavaScript."

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
      text: Guide
      link: /guide/
    - theme: alt
      text: Code source
      link: https://github.com/Oignontom8283/nwasabi
features:
  - title: Dans le navigateur ou en local
    details: L'émulateur web, un serveur local qui recharge l'application à chaque compilation, une ligne de commande pour les scripts et une bibliothèque JavaScript.
    link: /guide/
  - title: Un vrai débogueur
    details: Points d'arrêt, pile exacte, variables, expressions en C, remontée dans le temps et GDB, pour le C, le C++ et le Rust.
    link: /guide/deboguer-c-cpp-rust
---
