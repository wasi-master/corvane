# Build, test and install a small C project.
.DEFAULT_GOAL := all
SHELL := /bin/bash

CC      ?= cc
CFLAGS  := -std=c17 -Wall -Wextra -O2
PREFIX  ?= /usr/local
VERSION := $(shell git describe --tags --always 2>/dev/null || echo dev)

SRC_DIR := src
BUILD   := build
SOURCES := $(wildcard $(SRC_DIR)/*.c)
OBJECTS := $(patsubst $(SRC_DIR)/%.c,$(BUILD)/%.o,$(SOURCES))
TARGET  := $(BUILD)/sample

ifeq ($(DEBUG),1)
  CFLAGS += -g -O0 -DDEBUG
else
  CFLAGS += -DNDEBUG
endif

.PHONY: all clean install test

all: $(TARGET)

$(TARGET): $(OBJECTS) | $(BUILD)
	$(CC) $(LDFLAGS) -o $@ $^

$(BUILD)/%.o: $(SRC_DIR)/%.c | $(BUILD)
	@echo "  CC  $<"
	$(CC) $(CFLAGS) -DVERSION=\"$(VERSION)\" -MMD -c $< -o $@

$(BUILD):
	mkdir -p $@

test: all
	@for t in tests/*.sh; do bash "$$t" || exit 1; done

install: $(TARGET)
	install -m 0755 $< $(DESTDIR)$(PREFIX)/bin/

clean:
	$(RM) -r $(BUILD)

-include $(OBJECTS:.o=.d)
