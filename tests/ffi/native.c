#if defined(_WIN32)
#define FIXTURE_EXPORT __declspec(dllexport)
#else
#define FIXTURE_EXPORT __attribute__((visibility("default")))
#endif
#include <stdint.h>
static int64_t stored;
FIXTURE_EXPORT int32_t fixture_echo32(int32_t value) { return value; }
FIXTURE_EXPORT int64_t fixture_echo64(int64_t value) { return value; }
FIXTURE_EXPORT void fixture_store(int64_t value) { stored = value; }
FIXTURE_EXPORT int64_t fixture_load(void) { return stored; }
FIXTURE_EXPORT int64_t fixture_pair(int64_t a, int64_t b) { return a * 10 + b; }
