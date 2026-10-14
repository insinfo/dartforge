// POSIX producer used by the owner-thread synchronous callback probe.
#include <pthread.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdatomic.h>
#include <stdio.h>
#include <stdlib.h>
#include <time.h>
#include <unistd.h>

typedef struct {
  int32_t first;
  int32_t second;
} Pair8;

typedef struct {
  pthread_t thread;
  void *callback;
  int kind;
  int32_t buffer;
  _Atomic int done;
  int64_t integer;
  double real;
} Probe;

static pthread_t owner;
static _Atomic int watchdog_finished;

void sync_probe_sleep(int32_t millis) {
  struct timespec delay = {millis / 1000, (millis % 1000) * 1000000L};
  while (nanosleep(&delay, &delay) != 0) {}
}

static void *watchdog(void *unused) {
  (void)unused;
  sync_probe_sleep(3000);
  if (!atomic_load_explicit(&watchdog_finished, memory_order_acquire)) {
    fputs("synchronous callback probe exceeded its watchdog\n", stderr);
    _Exit(124);
  }
  return NULL;
}

int32_t sync_probe_initialize(void) {
  owner = pthread_self();
  atomic_store(&watchdog_finished, 0);
  pthread_t thread;
  if (pthread_create(&thread, NULL, watchdog, NULL) != 0) return 0;
  pthread_detach(thread);
  return 1;
}
void sync_probe_finish(void) { atomic_store(&watchdog_finished, 1); }
int32_t sync_probe_on_owner(void) { return pthread_equal(owner, pthread_self()); }

static void *invoke(void *argument) {
  Probe *p = argument;
  switch (p->kind) {
    case 0: p->integer = ((int32_t (*)(int32_t))p->callback)(p->buffer); break;
    case 1: p->real = ((double (*)(double))p->callback)(2.5); break;
    case 2: p->real = ((float (*)(float))p->callback)(1.25f); break;
    case 3: p->integer = ((_Bool (*)(_Bool))p->callback)(true); break;
    case 4: p->integer = ((void *(*)(void *))p->callback)(&p->buffer) == &p->buffer; break;
    case 5: ((void (*)(int32_t))p->callback)(p->buffer); p->integer = 1; break;
    case 6: p->integer = ((int32_t (*)(int32_t *))p->callback)(&p->buffer); break;
    case 7: {
      Pair8 pair = {p->buffer, 5};
      p->integer = ((int32_t (*)(Pair8))p->callback)(pair);
      break;
    }
    default: p->integer = -999;
  }
  atomic_store_explicit(&p->done, 1, memory_order_release);
  return NULL;
}

void *sync_probe_start(void *callback, int32_t kind, int32_t value) {
  Probe *p = calloc(1, sizeof(Probe));
  if (!p) return NULL;
  p->callback = callback;
  p->kind = kind;
  p->buffer = value;
  atomic_init(&p->done, 0);
  if (pthread_create(&p->thread, NULL, invoke, p) != 0) {
    free(p);
    return NULL;
  }
  return p;
}
int32_t sync_probe_done(void *state) {
  return atomic_load_explicit(&((Probe *)state)->done, memory_order_acquire);
}
int64_t sync_probe_integer(void *state) { return ((Probe *)state)->integer; }
double sync_probe_real(void *state) { return ((Probe *)state)->real; }
// Never join or reclaim storage while the native caller is still waiting.
int32_t sync_probe_destroy(void *state) {
  Probe *p = state;
  if (!sync_probe_done(p)) return 0;
  pthread_join(p->thread, NULL);
  free(p);
  return 1;
}
