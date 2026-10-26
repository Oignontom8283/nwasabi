// Colore l'écran selon les touches enfoncées : rouge = haut, vert = bas, bleu = gauche, blanc = droite,
// jaune = OK, noir = rien (ou plusieurs touches à la fois). Sert à vérifier de bout en bout qu'une touche
// (venue du clavier, d'un geste ou d'un mapping) arrive bien jusqu'à l'application.
#include "app.h"
TEST_APP("Keys");

int main(void) {
  for (;;) {
    eadk_keyboard_state_t keys = eadk_keyboard_scan();
    eadk_color_t color = eadk_color_black;
    int count = 0;
    if (eadk_keyboard_key_down(keys, eadk_key_up)) { color = eadk_color_red; count++; }
    if (eadk_keyboard_key_down(keys, eadk_key_down)) { color = eadk_color_green; count++; }
    if (eadk_keyboard_key_down(keys, eadk_key_left)) { color = eadk_color_blue; count++; }
    if (eadk_keyboard_key_down(keys, eadk_key_right)) { color = eadk_color_white; count++; }
    if (eadk_keyboard_key_down(keys, eadk_key_ok)) { color = 0xFFE0; count++; }
    if (count != 1) color = eadk_color_black;
    eadk_display_push_rect_uniform(eadk_screen_rect, color);
    eadk_display_wait_for_vblank();
  }
}
