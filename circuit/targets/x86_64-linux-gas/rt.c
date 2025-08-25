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

struct InitData {
  uintptr_t circ_index;
  uintptr_t *data;
  uintptr_t argc;
};

extern uintptr_t _init_data_size;
extern struct InitData _init_data[];
extern const char _lib_strtab[];
extern uintptr_t _func_table[];
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

size_t load_library(struct Library lib, size_t base_index) {
  for (size_t i = 0; i < lib.circ_count; i++) {
    CKT_INFO("loaded '%s' from '%s' (n: %04zx)\n", lib.circs[i].name, lib.name,
             base_index + i);
    _func_table[2 * (base_index + i)] = (uintptr_t)lib.circs[i].initialise;
    _func_table[2 * (base_index + i) + 1] = (uintptr_t)lib.circs[i].tick;
  }

  base_index += lib.circ_count;
  for (size_t i = 0; i < lib.library_count; i++) {
    CKT_INFO("discovered module '%s' in %s\n", lib.libraries[i].name, lib.name);
    base_index = load_library(lib.libraries[i], base_index);
  }

  return base_index;
}

void load_libraries(void) {
  size_t library_index = 0;
  size_t circ_index = 0;
  const char *lib_name = _lib_strtab;
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

    CKT_INFO("initialised library '%s'\n", lib_name);

    struct Library lib = result.lib;
    circ_index = load_library(lib, circ_index);

    lib_name += strlen(lib_name) + 1;
    library_index++;
  }

  if (library_index == 0)
    return;

  CKT_INFO("tick table: %zu entries\n", circ_index);
  CKT_INFO("      n           init           tick\n");
  for (size_t i = 0; i < circ_index; i++) {
    CKT_INFO("  [%04zx] %p %p\n", i, _func_table[2 * i],
             _func_table[2 * i + 1]);
  }
}

void initialise_circs(void) {
  if (_init_data_size == 0)
    return;

  CKT_INFO("initialising %zu circuits\n", _init_data_size);
  uint8_t *data_ptr = (uint8_t *)_init_data;
  for (size_t i = 0; i < _init_data_size; i++) {
    struct InitData data = *(struct InitData *)data_ptr;
    InitialiseFn init = (InitialiseFn)_func_table[2 * data.circ_index];

    data_ptr += sizeof(struct InitData);

    init(data.data, data.argc, (struct ConstValue *)(data_ptr));

    data_ptr += data.argc * sizeof(struct ConstValue);
  }
}

int main(void) {
  LOGGER = (struct Logger){
      .name = "cktrt",
      .level = ALL,
      .file = stderr,
  };

  CKT_INFO("=============== BEGIN INIT ===============\n");

  setup_signals();
  load_libraries();
  initialise_circs();

  CKT_INFO("===============  END INIT  ===============\n\n");

  for (;;) {
    _trampoline();
    usleep(100000);

    RUNTIME_STATE.ticks++;
  }
}
