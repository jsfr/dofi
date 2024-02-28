install_path := "/opt/homebrew/bin"
build_path := "./target/release"
binary := "dofi"

default:
	@just build

lint:
	cargo clippy --all-targets --all-features -- -W clippy::pedantic

upgrade:
	cargo update
	cargo upgrade

test:
	cargo test

build:
	cargo build --release

install: test build
    cp {{build_path}}/{{binary}} {{install_path}}

uninstall:
    rm {{install_path}}/{{binary}}
