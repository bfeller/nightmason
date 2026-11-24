# Nightmason build.
#   make            host build (runs headless with NM_FB=<file>; see README)
#   make cross      static aarch64 binary for the Cardputer Zero
#   make deploy     install into the APPLaunch launcher on the device (DEVICE=cardputer)
#   make sprites    repack assets/sprites.bin + src/sprites.rs from assets/src
DEVICE ?= cardputer
TARGET := aarch64-unknown-linux-musl
BIN    := target/$(TARGET)/release/nightmason
APP    := /usr/share/APPLaunch

.PHONY: all cross deploy sprites clean
all:
	cargo build --release
cross:
	cargo build --release --target $(TARGET)
	@ls -l $(BIN)
sprites:
	python3 tools/pack_sprites.py
deploy: cross
	scp -q $(BIN) packaging/nightmason.desktop assets/icon_100.png $(DEVICE):/tmp/
	ssh $(DEVICE) 'sudo install -m 755 /tmp/nightmason $(APP)/bin/nightmason && \
	  sudo install -m 644 /tmp/nightmason.desktop $(APP)/applications/nightmason.desktop && \
	  sudo install -m 644 /tmp/icon_100.png $(APP)/share/images/nightmason.png && \
	  systemctl --user restart APPLaunch'
clean:
	cargo clean
