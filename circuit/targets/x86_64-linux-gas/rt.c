#include <dlfcn.h>
#include <errno.h>
#include <signal.h>
#include <stdarg.h>
#include <stdint.h>
#include <string.h>
#include <unistd.h>

#include "circuit/extlib.h"

#define _CIRCUIT_RT_IMPLEMENTATION
#include "circuit/rt.h"

#ifndef MAX_LIBRARY_COUNT
#define MAX_LIBRARY_COUNT 1024
#endif

#ifndef usize
#define usize uint64_t
#endif

extern const char _lib_strtab;
extern uintptr_t _tick_table;
extern void _trampoline(void);

void *LIBRARY_HANDLES[MAX_LIBRARY_COUNT] = {0};

struct RuntimeState RUNTIME_STATE = {0};
struct Logger LOGGER = {0};

void exit(int status) {
  for (size_t i = 0; i < MAX_LIBRARY_COUNT; i++) {
    if (LIBRARY_HANDLES[i]) {
      dlclose(LIBRARY_HANDLES[i]);
      LIBRARY_HANDLES[i] = NULL;
    }
  }

  _exit(status);
}

void sigint_handler(int signum) {
  (void)signum;
  CKT_INFO("received SIGINT, exiting...\n");
  exit(0);
}

void setup_signals(void) {
  int retval = sigaction(
      SIGINT, &(struct sigaction){.sa_handler = &sigint_handler, .sa_flags = 0},
      NULL);

  if (retval < 0) {
    CKT_ERROR("failed to set up signal handler for SIGINT: %s\n",
              strerror(errno));
    exit(1);
  }
}

void load_libraries(void) {
  size_t library_index = 0;
  size_t circ_index = 0;
  const char *lib_name = &_lib_strtab;
  while (*lib_name) {
    void *handle = dlopen(lib_name, RTLD_LAZY | RTLD_LOCAL);
    if (!handle) {
      CKT_ERROR("failed to open shared library '%s': %s\n", lib_name,
                dlerror());
      exit(1);
    }

    if (library_index >= MAX_LIBRARY_COUNT) {
      CKT_ERROR("too many libraries loaded, max is %zu\n", MAX_LIBRARY_COUNT);
      exit(1);
    }

    LIBRARY_HANDLES[library_index] = handle;

    InitFn init = dlsym(handle, "init");
    if (!init) {
      const char *error = dlerror();
      if (!error) {
        CKT_ERROR("invalid init function in '%s'\n", lib_name);
      } else {
        CKT_ERROR("failed to find init function in '%s': %s\n", lib_name,
                  error);
      }
      exit(1);
    }

    char buffer[64] = {0};
    struct InitResult result = init(0, &buffer);

    if (result.error) {
      CKT_ERROR("failed to initialize library '%s': %s\n", lib_name,
                result.error);
      exit(1);
    }

    struct Library lib = result.lib;

    for (size_t i = 0; i < lib.circ_count; i++) {
      CKT_INFO("loaded '%s' from '%s' (tick: %p)\n", lib.circs[i].name,
               lib.name, lib.circs[i].tick);
      (&_tick_table)[circ_index] = (uintptr_t)lib.circs[i].tick;
      circ_index++;
    }

    lib_name += strlen(lib_name) + 1;
    library_index++;
  }
}

int main(void) {
  LOGGER = (struct Logger){
      .name = "cktrt",
      .level = ALL,
      .file = stderr,
  };

  setup_signals();
  load_libraries();

  for (;;) {
    _trampoline();
    usleep(100000);

    RUNTIME_STATE.ticks++;
  }
}
