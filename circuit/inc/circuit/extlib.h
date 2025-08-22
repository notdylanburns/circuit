#include <stdint.h>
#include <stdio.h>

enum ConstType {
  CT_BOOL,
  CT_INT,
  CT_ENUM,
};

#define CT_NONE ((const enum ConstType)(CT_ENUM + 1))

struct ConstValue {
  const enum ConstType type;
  union {
    uint8_t none;
    uint8_t boolean;
    intptr_t integer;
    const char *enum_value;
  } value;
};

enum PinDirection {
  PIN_DIRECTION_INPUT,
  PIN_DIRECTION_OUTPUT,
  PIN_DIRECTION_TRANSPUT,
};

enum TickBehaviour {
  TICK_ALWAYS,
  TICK_ON_CHANGE,
  TICK_ONCE,
};

struct ArgDescriptor {
  const char *name;
  const enum ConstType type;
  const struct ConstValue default_value;
};

struct Pin {
  const char *name;
  const uintptr_t width;
  const enum PinDirection direction;
};

struct CircMeta {
  const uintptr_t pin_count;
  const struct Pin *pins;
  const enum TickBehaviour tick_behaviour;
};

struct CircDescriptor {
  const char *name;
  const uintptr_t arg_count;
  const struct ArgDescriptor *args;
  void (*const initialise)(void *mem, void *_args);
  void (*const tick)(void *circ, void *_state);
  struct CircMeta (*const get_meta)(void *circ);
};

struct Library {
  const char *name;
  const uintptr_t circ_count;
  const struct CircDescriptor *circs;
  const uintptr_t const_count;
  const void *consts;
  const uintptr_t enum_count;
  const void *enums;
  const uintptr_t library_count;
  const struct Library *libraries;
};

struct InitResult {
  const struct Library lib;
  const char *error;
};

typedef struct InitResult (*InitFn)(uintptr_t, const void *);
