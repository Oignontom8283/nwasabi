import { getCurrentInstance, inject, onActivated, onDeactivated } from "vue";
import type { Ref } from "vue";

/**
 * Relève périodiquement (`task` est asynchrone : elle parle au Worker) sans jamais empiler deux relevés : si le précédent
 * n'est pas fini, on saute ce tour. Rend la fonction qui arrête.
 *
 * Appelée depuis un composant, la relève se met en pause quand ce composant est masqué (un onglet gardé en mémoire par
 * `KeepAlive`) et repart, avec un relevé immédiat, quand il revient : un onglet caché ne fait aucun travail.
 */
export function poll(task: () => Promise<void> | void, ms: number): () => void {
  let busy = false;
  let paused = false;
  const owner = getCurrentInstance();
  // Le panneau latéral reste en mémoire quand on le ferme : il dit s'il est ouvert, et ce qu'il contient attend d'être revu.
  const panelOpen = owner ? inject<Ref<boolean> | null>("panelActive", null) : null;
  const run = async () => {
    if (busy || paused || (panelOpen && !panelOpen.value)) return;
    busy = true;
    try {
      await task();
    } finally {
      busy = false;
    }
  };
  if (owner) {
    onDeactivated(() => (paused = true), owner);
    onActivated(() => {
      paused = false;
      void run();
    }, owner);
  }
  void run();
  const timer = setInterval(run, ms);
  return () => clearInterval(timer);
}
