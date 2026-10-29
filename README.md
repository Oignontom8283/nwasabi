<div align="center">

<img src="assets/logo/logo-bordered.svg" alt="nwasabi logo" width="128" height="128">

# Nwasabi !!

**Run NumWorks `.nwa` apps anywhere: in your browser, in Node.js or from the command line.**

[![crates.io](https://img.shields.io/crates/v/nwasabi?logo=rust&color=74AD3A)](https://crates.io/crates/nwasabi)
![docs](https://img.shields.io/badge/docs-coming%20soon-74AD3A)
[![crate size](https://img.shields.io/crates/size/nwasabi?logo=rust&color=74AD3A)](https://crates.io/crates/nwasabi)
[![npm](https://img.shields.io/npm/v/%40nwasabi%2Fjs?logo=npm&label=%40nwasabi%2Fjs&color=74AD3A)](https://www.npmjs.com/package/@nwasabi/js)
[![License: AGPL-3.0](https://img.shields.io/badge/license-AGPL--3.0-74AD3A)](./LICENSE)

</div>

## AI Usage Declaration

En raison de la complexité et de l'envergure de ce projet, l'utilisation de l'IA y est autorisée et utilisée pour son développement.

- Il n'est pas absolument nécessaire de signaler l'utilisation de l'IA dans une contribution.
- Vos commits ne doivent pas contenir de co-auteurs IA qui polluent git.
- Vos branches Git doivent être propres et pas un spaghetti visuel et incompréhensible (ce que l'IA a tendance à faire). Réécrivez votre historique Git si nécessaire. L'IA sait très bien le faire si on lui demande.
- Vous êtes l'auteur et le seul responsable de vos contributions! pas un agent IA.
- Les instructions de contributing incluent des instructions spécifiques pour les contributeurs que l'IA doit respecter. Donnez-lui ce jeu d'instructions comme obligation, le code résultant devrait être correct. Mais uniquement techniquement, l'utilisabilité et l'abstraction ne sont pas ses forts, c'est votre taff.
- Essayez tout de même, d'écrire vous-même vos PR, au moins le corps. L'IA peut faire les spécifications de modifications techniques. Ce n'est pas un problème. 

Faites tout de même attention à ce qui est généré par l'IA : vérifiez toujours que le code fonctionne et qu'il passe les suites de tests et le formateur Clippy.
Prendre le temps de relire ce qui a été généré reste la meilleure pratique dès que vous en avez la capacité :)

Ces règles sur l'IA valent uniquement pour ce dépôt, en raison de sa nature.

## License & Disclaimer

This project is licensed under the [GNU Affero General Public License
version 3](./LICENSE) (AGPL-3.0-only), with an additional permission
described in [LINKING-EXCEPTION](./LINKING-EXCEPTION).

In short:

- You can use this project as a library and combine it with your own
  code, including by compiling everything into a single binary or
  WebAssembly module, as long as your code only uses the public API.
  Your code can then be under any license.
- If you modify the core itself (including by adding new code that
  extends or changes the emulation), your modifications must remain
  open under the AGPL, including when the software is provided over a
  network.
- Modified versions are asked to keep the additional permission, so
  that they remain usable by others under the same conditions.

In case of doubt, the text of [`LICENSE`](./LICENSE) and [`LINKING-EXCEPTION`](./LINKING-EXCEPTION)
prevails over this summary.

### Additional components

Some components of this repository are redistributed under a license
different from the one above:

- **Nwasabi Pixels**, the font used by `draw_string`
  (`src/kernel/font/` and `tools/font/`): **MIT**.
- **`@nwasabi/js`** and **`@nwasabi/wasm`**,
  the JavaScript and WebAssembly bindings for Nwasabi: **MIT** or **Apache-2.0**.

> [!NOTE]
> This project is a community-driven, independent
> effort. It is not affiliated with, endorsed by, or sponsored by
> NumWorks, Epsilon, or any of their affiliates. All product names and
> trademarks are the property of their respective owners and are used
> here for identification purposes only.