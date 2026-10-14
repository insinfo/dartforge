// The pthread start routine has its actual C signature; Dart callbacks return void.
#include <pthread.h>
#include <stddef.h>

typedef void (*callback_fn)(void *);
struct invocation { callback_fn callback; };
static void *invoke(void *data) {
    struct invocation *invocation = data;
    invocation->callback(NULL);
    return NULL;
}
int dartforge_test_invoke_callback_on_thread(callback_fn callback) {
    pthread_t thread;
    struct invocation invocation = {callback};
    int status = pthread_create(&thread, NULL, invoke, &invocation);
    if (status != 0) return status;
    return pthread_join(thread, NULL);
}

#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>

typedef int32_t (*integer_callback_fn)(int32_t);
struct integer_invocation {
    integer_callback_fn callback;
    int32_t argument;
    int32_t result;
};
static void *invoke_integer(void *data) {
    struct integer_invocation *invocation = data;
    invocation->result = invocation->callback(invocation->argument);
    return NULL;
}
int dartforge_test_invoke_integer_on_thread(integer_callback_fn callback,
                                           int32_t argument, int32_t *result) {
    pthread_t thread;
    struct integer_invocation invocation = {callback, argument, 0};
    int status = pthread_create(&thread, NULL, invoke_integer, &invocation);
    if (status != 0) return status;
    status = pthread_join(thread, NULL);
    if (status == 0) *result = invocation.result;
    return status;
}

static integer_callback_fn exit_callback;
static int32_t exit_exceptional;
static void verify_after_finalize(void) {
    if (exit_callback(5) != exit_exceptional) {
        fputs("atexit callback retirement failed\n", stderr);
        fflush(stderr);
        _Exit(71);
    }
    fputs("atexit callback exceptional return verified\n", stderr);
    fflush(stderr);
}
int dartforge_test_register_atexit(integer_callback_fn callback,
                                  int32_t exceptional) {
    exit_callback = callback;
    exit_exceptional = exceptional;
    return atexit(verify_after_finalize);
}
