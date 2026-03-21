PYTHON ?= python3
PYTEST ?= pytest
CARGO_DIR := ../../
LIB_DIR := $(CARGO_DIR)/target/debug

UNAME := $(shell uname -s)
ifeq ($(UNAME),Darwin)
  DYLIB_VAR := DYLD_LIBRARY_PATH
else
  DYLIB_VAR := LD_LIBRARY_PATH
endif

.PHONY: build test lint clean

build:
	cd $(CARGO_DIR) && cargo build

test:
	$(DYLIB_VAR)=$(LIB_DIR) $(PYTEST) tests/ -v

lint:
	mypy wispers_connect/
	ruff check wispers_connect/ tests/

clean:
	find . -type d -name __pycache__ -exec rm -rf {} +
