.PHONY: all build install start stop restart clean update

all: build

build:
	cargo build --release

install: build
	./scripts/install-service.sh
	# Link the CLI to /usr/local/bin for easy access
	sudo ln -sf "$$(pwd)/target/release/mosaic-msg" /usr/local/bin/mosaic-msg

start:
	launchctl load ~/Library/LaunchAgents/io.mosaic.daemon.plist || true
	launchctl start io.mosaic.daemon

stop:
	launchctl stop io.mosaic.daemon
	launchctl unload ~/Library/LaunchAgents/io.mosaic.daemon.plist || true

restart: stop start

clean:
	cargo clean
	rm -f /tmp/mosaic.sock
	rm -f /tmp/mosaic.out.log
	rm -f /tmp/mosaic.err.log

update:
	git pull
	make install
	make restart
