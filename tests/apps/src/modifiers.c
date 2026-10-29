// L'état de shift et d'alpha que les applications qui gèrent leur propre clavier (KhiCAS) lisent et écrivent.
#include "app.h"
#include "svc.h"
TEST_APP("Modifiers");

int main(void) {
  int failures = 0;
  CHECK(svc_shift_alpha() == 0);
  svc_set_shift_alpha(4);  // alpha verrouillé
  CHECK(svc_shift_alpha() == 4);
  svc_set_shift_alpha(2);  // alpha
  CHECK(svc_shift_alpha() == 2);
  svc_set_shift_alpha(1);  // shift
  CHECK(svc_shift_alpha() == 1);
  svc_set_shift_alpha(5);  // shift et alpha verrouillé
  CHECK(svc_shift_alpha() == 5);
  svc_set_shift_alpha(0);
  CHECK(svc_shift_alpha() == 0);
  return failures;
}
