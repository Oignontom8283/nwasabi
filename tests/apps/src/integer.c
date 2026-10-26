// Un mélange d'instructions entières comme un compilateur en produit : division, multiplications
// longues, champs de bits, rotations, inversions, saturations, tables de branchement.
#include "app.h"
TEST_APP("Integer");

// Les entrées passent par des volatile : le compilateur ne peut pas calculer les résultats lui-même.
static volatile int32_t a = -7, b = 2;
static volatile uint32_t u = 0x80000001u, v = 3;

static int switch_table(int x) {
  switch (x) {
    case 0: return 11;  case 1: return 22;  case 2: return 33;  case 3: return 44;
    case 4: return 55;  case 5: return 66;  case 6: return 77;  default: return -1;
  }
}

int main(void) {
  int failures = 0;
  CHECK(a / b == -3);                 // sdiv
  CHECK(a % b == -1);
  CHECK(u / v == 0x2aaaaaabu);        // udiv
  CHECK(((int64_t)a * (int64_t)b) == -14);                   // smull
  CHECK(((uint64_t)u * (uint64_t)v) == 0x180000003ull);      // umull
  CHECK(__builtin_clz(v) == 30);
  CHECK(__builtin_bswap32(u) == 0x01000080u);                // rev
  CHECK((u >> 4 | u << 28) == 0x18000000u);                  // ror
  CHECK(((u >> 3) & 0xff) == 0x00);                          // ubfx
  CHECK((a >> 1) == -4);                                     // asr
  CHECK(((int32_t)(a << 29) >> 29) == 1);                    // sbfx
  CHECK(switch_table(a + 12) == 66 && switch_table(v) == 44 && switch_table(40) == -1);
  return failures;
}
