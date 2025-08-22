#ifndef _CIRCUIT_RT_H_GUARD
#define _CIRCUIT_RT_H_GUARD

#include <limits.h>
#include <stdarg.h>
#include <stdint.h>
#include <stdio.h>

#define NONE 0
#define FATAL 10
#define ERROR 20
#define WARN 30
#define INFO 40
#define DEBUG 50
#define ALL INT_MAX

struct Logger {
  const char *name;
  FILE *file;
  int level;
};

extern void circuit_log(struct Logger *logger, int level, const char *fmt, ...);

#define CKT_LOG(level, fmt, ...) circuit_log(&LOGGER, level, fmt, ##__VA_ARGS__)
#define CKT_FATAL(fmt, ...) circuit_log(&LOGGER, FATAL, fmt, ##__VA_ARGS__)
#define CKT_ERROR(fmt, ...) circuit_log(&LOGGER, ERROR, fmt, ##__VA_ARGS__)
#define CKT_WARN(fmt, ...) circuit_log(&LOGGER, WARN, fmt, ##__VA_ARGS__)
#define CKT_INFO(fmt, ...) circuit_log(&LOGGER, INFO, fmt, ##__VA_ARGS__)
#define CKT_DEBUG(fmt, ...) circuit_log(&LOGGER, DEBUG, fmt, ##__VA_ARGS__)

struct RuntimeState {
  uintmax_t ticks;
};

extern struct RuntimeState RUNTIME_STATE;

#ifdef _CIRCUIT_RT_IMPLEMENTATION
void circuit_log(struct Logger *logger, int level, const char *fmt, ...) {
  if (level > logger->level) {
    return;
  }

  switch (level) {
  case FATAL:
    fprintf(logger->file, "[FATAL] ");
    break;
  case ERROR:
    fprintf(logger->file, "[ERROR] ");
    break;
  case WARN:
    fprintf(logger->file, "[WARN] ");
    break;
  case INFO:
    fprintf(logger->file, "[INFO] ");
    break;
  case DEBUG:
    fprintf(logger->file, "[DEBUG] ");
    break;
  }

  if (logger->name) {
    fprintf(logger->file, "%s: ", logger->name);
  }

  va_list args;
  va_start(args, fmt);

  vfprintf(logger->file, fmt, args);

  va_end(args);
}
#endif

#endif
