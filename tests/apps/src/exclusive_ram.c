// ldrex sur une variable de la pile : BusFault sur la calculatrice (SRAM partageable, pas de
// moniteur d'exclusivité global). L'application ne doit jamais atteindre le « return 0 ».
#include "app.h"
TEST_APP("ExclusiveRam");

int main(void) {
  volatile uint32_t value = 1;
  uint32_t loaded;
  __asm__ volatile("ldrex %0, [%1]" : "=r"(loaded) : "r"(&value) : "memory");
  return 0;
}
