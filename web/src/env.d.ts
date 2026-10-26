// Ce que Vite apporte au code et que TypeScript ne peut pas deviner.

/// <reference types="vite/client" />

/** Texte de la fenêtre « i », fixé à la compilation (`NWASABI_INFO`, voir `vite.config.ts`) ; vide sans texte. */
declare const __NWASABI_INFO__: string;
declare const __NWASABI_INFO_TITLE__: string;

/** Le module WebAssembly construit (alias de `vite.config.ts`). */
declare module "nwasabi.wasm?url" {
  const url: string;
  export default url;
}

declare module "*.vue" {
  import type { DefineComponent } from "vue";
  const component: DefineComponent<object, object, unknown>;
  export default component;
}
