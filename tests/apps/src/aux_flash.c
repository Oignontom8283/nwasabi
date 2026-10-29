// Une application qui lit des données auxiliaires en flash, comme le lanceur de KhiCAS : un programme à 0x90400000 et une table à
// 0x90260000. Les tests (tests/aux_flash.rs) les placent avec un fichier .nfd. Sans elles, l'application rend le numéro de la
// première vérification ratée.
#include "app.h"
TEST_APP("AuxFlash");

#define PROGRAM ((const volatile uint8_t *)0x90400000)
#define TABLE ((const volatile uint32_t *)0x90260000)

int main(void) {
  int failures = 0;
  // En-tête du programme : la signature « NWAUX », puis du code Thumb à l'adresse +8 : movs r0, #42 ; bx lr.
  CHECK(PROGRAM[0] == 'N' && PROGRAM[1] == 'W' && PROGRAM[2] == 'A' && PROGRAM[3] == 'U' && PROGRAM[4] == 'X');
  // La table : 16 mots, le mot n vaut 0x1000 + 7n ; on les additionne.
  uint32_t sum = 0;
  for (int i = 0; i < 16; i++) {
    sum += TABLE[i];
  }
  CHECK(sum == 16 * 0x1000 + 7 * (15 * 16 / 2));
  // Le code placé en flash s'exécute : on l'appelle (le bit 0 de l'adresse dit « Thumb »).
  int (*routine)(void) = (int (*)(void))(0x90400008 | 1);
  CHECK(routine() == 42);
  return failures;
}
