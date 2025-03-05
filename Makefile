build:
	cargo build

clean:
	cargo clean

install:
	cargo install --path .

release:
	cargo build --release