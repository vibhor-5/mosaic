.PHONY: all build install start stop restart clean update

all: build

build:
	cargo build --release

install: build
	./scripts/install-service.sh
	# Link the CLI to /usr/local/bin for easy access
	sudo ln -sf "$$(pwd)/target/release/mosaic-msg" /usr/local/bin/mosaic-msg
	@echo ""
	@echo "============================================================"
	@echo "Mosaic supports instant space switching via a Scripting Addition."
	@echo "This requires System Integrity Protection (SIP) to be partially disabled."
	@echo "If you have already disabled SIP, you can inject it now."
	@echo "============================================================"
	@read -p "Inject Scripting Addition into Dock.app? [y/N] " ans; \
	if [ "$$ans" = "y" ] || [ "$$ans" = "Y" ]; then \
		./scripts/install-sa.sh; \
	else \
		echo "Skipping Scripting Addition injection."; \
	fi

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
