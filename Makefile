.PHONY: build test install clean release

VERSION := 1.0.0

build:
	cargo build --release

test:
	cargo test --all-features

install: build
	sudo cp target/release/vedc /usr/local/bin/

uninstall:
	sudo rm -f /usr/local/bin/vedc

clean:
	cargo clean
	rm -rf dist/

release:
	cargo build --release
	mkdir -p release
	cp target/release/vedc release/vedc-${VERSION}
	cp -r examples release/
	cp README.md release/
	cp LICENSE release/
	tar -czf vedc-${VERSION}.tar.gz -C release .

# Development helpers
run-hello:
	cargo run -- run examples/hello.ved

run-counter:
	cargo run -- run examples/counter.ved

build-examples:
	cargo run -- build examples/hello.ved --target web --out dist/hello
	cargo run -- build examples/counter.ved --target web --out dist/counter
	cargo run -- build examples/todo/client.ved --target web --out dist/todo-client
	cargo run -- build examples/todo/server.ved --target server --out dist/todo-server

.PHONY: run-hello run-counter build-examples
