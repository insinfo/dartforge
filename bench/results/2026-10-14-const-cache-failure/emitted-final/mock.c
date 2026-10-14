
#include <assert.h>
#include <stdint.h>
#include <stdlib.h>
#include <string.h>
#include <stdio.h>
static _Alignas(8) unsigned char context[512];
static uint64_t area[2048];
static int allocations, marks;
static void *allocated[8];
void *dartforge_context(void) { return context; }
void *context_alias(void) __asm__("_df.context");
void *context_alias(void) { return context; }
void *area_alias(void) __asm__("_df.get_area");
void *area_alias(void) { return area; }
int64_t box_int(int64_t value) __asm__("_df.box_int");
int64_t box_int(int64_t value) { assert(value == 7); return (value << 1) | 1; }
void dartforge_stack_overflow(void) { context[0] = 1; }
uint8_t dartforge_exception_pending(void) { return context[0]; }
void dartforge_exception_clear(void) { context[0] = 0; }
int64_t dartforge_object_new_t(int64_t cid, int64_t fields, void *table) {
  (void)table;
  unsigned char *p = calloc(1, 16 + 8 * fields); assert(p);
  *(uint64_t *)p = (uint64_t)cid << 32;
  allocated[allocations++] = p;
  if (allocations == 1) *(uintptr_t *)(context + 376) = UINTPTR_MAX;
  return (intptr_t)p + 2;
}
void barrier(int64_t a, int64_t b) __asm__("_df.barrier");
void barrier(int64_t a, int64_t b) { (void)a; (void)b; }
int64_t dartforge_canonical_constant(int64_t value) { return value; }
void dartforge_gc_global_root(int64_t root, int64_t value) { (void)root; (void)value; }
void dartforge_mark_constant(int64_t value, int64_t getter) { assert(value); (void)getter; marks++; }
void dartforge_effects_before(void *name, int64_t length, int64_t mask) { (void)name; (void)length; (void)mask; }
void dartforge_effects_after(void) {}
void *table_0(void) __asm__("_df.mt.main$2edart.Leaf"); void *table_0(void) { return 0; }
void *table_1(void) __asm__("_df.mt.main$2edart.Parent"); void *table_1(void) { return 0; }
void *table_2(void) __asm__("_df.mt.$type"); void *table_2(void) { return 0; }

int64_t parent(void) __asm__("_cache_test_parent");
int main(void) {
  assert(parent() == 0); assert(context[0] == 1); assert(allocations == 1); assert(marks == 0);
  for (int i = 0; i < 2048; i++) assert(area[i] == 0);
  assert(*(uintptr_t *)(context + 8) == 0);
  context[0] = 0; *(uintptr_t *)(context + 376) = 0;
  int64_t value = parent(); assert(value != 0); assert(context[0] == 0);
  assert(allocations == 3); assert(marks == 2);
  int64_t child = *(int64_t *)((intptr_t)value - 2 + 16); assert(child != 0);
  assert(*(int64_t *)((intptr_t)child - 2 + 16) == 7);
  assert(parent() == value); assert(allocations == 3); assert(marks == 2);
  assert(*(uintptr_t *)(context + 8) == 0);
  puts("child-guard-failed; parent-unpublished; retry-valid; warm-identity-preserved");
  for (int i = 0; i < allocations; i++) free(allocated[i]);
  return 0;
}
