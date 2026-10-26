// Virgule flottante en simple précision (le Cortex-M7 de la calculatrice n'a pas de double
// matériel) : arithmétique, racine, multiplication-addition fusionnée, conversions, comparaisons.
#include "app.h"
TEST_APP("Float");

static volatile float x = 1.5f, y = -2.25f, big = 3.0e9f;
static volatile int32_t n = -7;

int main(void) {
  int failures = 0;
  CHECK(x + y == -0.75f);                       // vadd
  CHECK(x * y == -3.375f);                      // vmul
  CHECK(x / y == -2.0f / 3.0f);                 // vdiv
  CHECK(__builtin_sqrtf(x * x * 4.0f) == 3.0f); // vsqrt
  CHECK(__builtin_fmaf(x, y, 1.0f) == -2.375f); // vfma
  CHECK((int32_t)y == -2);                      // vcvt.s32.f32 (vers zéro)
  CHECK((float)n == -7.0f);                     // vcvt.f32.s32
  CHECK((uint32_t)big == 3000000000u);          // vcvt.u32.f32
  CHECK(__builtin_fabsf(y) == 2.25f && -x == -1.5f);
  CHECK(x > y && y < x && !(x < y));
  float not_a_number = __builtin_nanf("");
  CHECK(not_a_number != not_a_number);          // comparaison non ordonnée
  return failures;
}
