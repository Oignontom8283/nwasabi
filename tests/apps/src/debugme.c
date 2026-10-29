// CFLAGS: -O0 -g
// Une application faite pour être déboguée : fonctions, paramètres, variables locales, structures, tableaux, énumération,
// récursion, bloc imbriqué, variables globales. Les tests lisent ses informations DWARF (lignes, variables, pile).
#include "app.h"
TEST_APP("Debug");

typedef enum { RED, GREEN = 5, BLUE } Color;
typedef struct { int x; int y; } Point;
typedef struct { Point corner; const char *name; Color color; uint8_t flags[3]; } Shape;

int global_counter = 7;
static Point origin = { 3, -4 };

__attribute__((noinline)) int add(int a, int b) {
  int sum = a + b;
  return sum;
}

__attribute__((noinline)) int factorial(int n) {
  if (n <= 1) {
    return 1;
  }
  int rest = factorial(n - 1);
  return n * rest;
}

__attribute__((noinline)) int fill(Shape *shape, int base) {
  int total = 0;
  for (int i = 0; i < 3; i++) {
    shape->flags[i] = (uint8_t)(base + i);
    total += shape->flags[i];
  }
  return total;
}

int main(void) {
  int failures = 0;
  Shape shape = { { 1, 2 }, "box", GREEN, { 0, 0, 0 } };
  CHECK(add(2, 3) == 5);
  CHECK(factorial(5) == 120);
  CHECK(fill(&shape, 10) == 33);
  global_counter += shape.corner.x + origin.x;
  return failures;
}

// Ce qui suit sert aux tests des expressions (conversions, tableaux à plusieurs dimensions, flottants, appels de fonctions) ; il est après
// `main` pour ne pas décaler les numéros de ligne que les tests attendent plus haut.
typedef int (*binary_op)(int, int);

int grid[2][3] = { { 1, 2, 3 }, { 4, 5, 6 } };
uint32_t words[4] = { 10, 20, 30, 40 };
float ratio = 2.5f;
double wide = -0.125;
binary_op chosen = add;

__attribute__((noinline)) int twice(int x) {
  return 2 * x;
}

__attribute__((noinline)) void bump(void) {
  global_counter += 1;
}
