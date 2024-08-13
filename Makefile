build:
	cargo build

fix:
	cargo fix --allow-dirty --allow-staged
	cargo clippy --fix --no-deps --allow-dirty --allow-staged
	cargo +nightly fmt