// ldrex sur une constante en flash : accepté (la flash n'est pas partageable). strex n'a pas de
// sens en flash, on s'en tient à la lecture.
#include "app.h"
TEST_APP("ExclusiveFlash");

static const uint32_t constant = 0xc0ffee;

int main(void) {
  int failures = 0;
  uint32_t loaded;
  __asm__ volatile("ldrex %0, [%1]\n\tclrex" : "=r"(loaded) : "r"(&constant) : "memory");
  CHECK(loaded == 0xc0ffee);
  return failures;
}
