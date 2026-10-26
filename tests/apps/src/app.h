// En-tête commun des applications de test : nom et niveau d'API exigés par le format .nwa.
#include <eadk.h>

#define TEST_APP(name)                                                     \
  const char eadk_app_name[] __attribute__((section(".rodata.eadk_app_name"))) = name; \
  const uint32_t eadk_api_level __attribute__((section(".rodata.eadk_api_level"))) = 0

// Ces applications rendent 0 si tout est correct, sinon le numéro de ligne de la première vérification ratée.
#define CHECK(cond) do { if (!(cond) && failures == 0) failures = __LINE__; } while (0)
