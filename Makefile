GREP ?=
INCLUDE ?=
INCLUDE_DEPS ?=

build:
	cargo build

clean:
	cargo clean

install:
	cargo install --path .

release:
	cargo build --release

run:
	cargo run --

run-grep:
	cargo run -- --grep $(GREP)

run-include:
	cargo run -- --include $(INCLUDE)

run-include-deps:
	cargo run -- --include-deps $(INCLUDE_DEPS)