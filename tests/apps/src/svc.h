// Les appels système du noyau d'Epsilon que le lanceur de KhiCAS utilise et que l'EADK n'expose pas : écrits à la main avec `svc`.
// Ils servent aux applications de test ; une vraie calculatrice les propose à toute application liée à l'EADK étendue.
#include <stdint.h>

// État de shift et d'alpha : 0 rien, 1 shift, 2 alpha, 4 alpha verrouillé (3 et 5 : shift avec alpha).
static inline uint32_t svc_shift_alpha(void) {
  register uint32_t r0 __asm__("r0");
  __asm__ volatile("svc #28" : "=r"(r0) : : "memory");
  return r0;
}

static inline void svc_set_shift_alpha(uint32_t status) {
  register uint32_t r0 __asm__("r0") = status;
  __asm__ volatile("svc #26" : "+r"(r0) : : "memory");
}

// Efface un secteur de la flash externe ; rend 1 si c'est fait, 0 si le noyau refuse (firmware, applications installées).
static inline uint32_t svc_flash_erase(uint32_t sector) {
  register uint32_t r0 __asm__("r0") = sector;
  register uint32_t r1 __asm__("r1") = 1;
  __asm__ volatile("svc #30" : "+r"(r0) : "r"(r1) : "memory");
  return r0 & 0xff;
}

// Programme la flash externe (les bits ne passent qu'à 0) ; rend 1 si c'est fait, 0 si le noyau refuse.
static inline uint32_t svc_flash_write(void *destination, const void *source, uint32_t length) {
  register uint32_t r0 __asm__("r0") = (uint32_t)destination;
  register uint32_t r1 __asm__("r1") = (uint32_t)source;
  register uint32_t r2 __asm__("r2") = length;
  register uint32_t r3 __asm__("r3") = 1;
  __asm__ volatile("svc #32" : "+r"(r0) : "r"(r1), "r"(r2), "r"(r3) : "memory");
  return r0 & 0xff;
}
