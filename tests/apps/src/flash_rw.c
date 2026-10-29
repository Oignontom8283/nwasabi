// Écrire dans la flash comme le fait KhiCAS pour garder ses scripts : effacer un secteur, y programmer des octets (les bits ne passent
// qu'à 0), relire. Le noyau refuse de toucher au firmware et aux applications installées.
#include "app.h"
#include "svc.h"
TEST_APP("FlashRW");

// Le secteur 120 est de 64 Kio, à 0x90700000 : loin du firmware et des applications, dans la zone libre.
#define SECTOR 120
#define AREA ((volatile uint32_t *)0x90700000)

int main(void) {
  int failures = 0;
  CHECK(svc_flash_erase(SECTOR) == 1);
  CHECK(AREA[0] == 0xffffffffu && AREA[1000] == 0xffffffffu);

  uint32_t first[2] = {0x12345678u, 0x0f0f0f0fu};
  CHECK(svc_flash_write((void *)AREA, first, sizeof first) == 1);
  CHECK(AREA[0] == 0x12345678u && AREA[1] == 0x0f0f0f0fu);

  // Réécrire sans effacer : seuls les bits à 0 comptent.
  uint32_t again = 0xff00ff00u;
  CHECK(svc_flash_write((void *)AREA, &again, sizeof again) == 1);
  CHECK(AREA[0] == (0x12345678u & 0xff00ff00u));

  // Effacer de nouveau remet tout à 1.
  CHECK(svc_flash_erase(SECTOR) == 1);
  CHECK(AREA[0] == 0xffffffffu);
  uint32_t last = 0xcafef00du;
  CHECK(svc_flash_write((void *)AREA, &last, sizeof last) == 1);

  // Le firmware et les applications sont protégés : le noyau refuse, rien ne change.
  CHECK(svc_flash_erase(9) == 0);                                   // 0x90010000
  CHECK(svc_flash_write((void *)0x90010000, &last, sizeof last) == 0);
  CHECK(svc_flash_erase(500) == 0);                                 // un secteur qui n'existe pas
  return failures;
}
