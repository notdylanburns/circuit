ROOT_DIR:=$(shell dirname $(realpath $(firstword $(MAKEFILE_LIST))))

AS ?= as
CC ?= cc
LD ?= ld

INC_DIR ?= $(ROOT_DIR)/../../inc/
ASFLAGS ?= -ggdb
CFLAGS ?= -Wall -Wextra -O0 -I$(INC_DIR)
CFLAGS += -ggdb -fsanitize=address -static-libasan
LDFLAGS ?=

TARGET := cktrt.o

_C_OBJS := $(addsuffix .o, $(wildcard *.c))
_ASM_OBJS := $(addsuffix .o, $(wildcard *.s))
OBJS := $(_C_OBJS) $(_ASM_OBJS)

build: $(TARGET)
.PHONY: build

$(TARGET): $(OBJS)
	$(CC) $(CFLAGS) -o $@ -r $^

%.c.o: %.c $(wildcard $(INC_DIR)/*/*.h)
	$(CC) $(CFLAGS) -c $< -o $@

%.s.o: %.s
	$(AS) $(ASFLAGS) -c $< -o $@

install:
	install -Dm644 $(TARGET) /usr/local/lib/$(TARGET)
