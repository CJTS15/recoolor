# Builds and installs Recoolor. Packages use DESTDIR; a per-user install can use
# PREFIX=~/.local (then ~/.local/bin must be on PATH for the desktop file).
PREFIX ?= /usr/local
DESTDIR ?=
APP_ID := io.github.cjts15.Recoolor
SHARE := $(DESTDIR)$(PREFIX)/share

.PHONY: build install uninstall

build:
	cargo build --release --locked

install:
	install -Dm755 target/release/recoolor $(DESTDIR)$(PREFIX)/bin/recoolor
	install -Dm644 data/$(APP_ID).desktop $(SHARE)/applications/$(APP_ID).desktop
	install -Dm644 data/$(APP_ID).metainfo.xml $(SHARE)/metainfo/$(APP_ID).metainfo.xml
	install -Dm644 data/icons/hicolor/scalable/apps/$(APP_ID).svg \
		$(SHARE)/icons/hicolor/scalable/apps/$(APP_ID).svg
	install -Dm644 data/icons/hicolor/symbolic/apps/$(APP_ID)-symbolic.svg \
		$(SHARE)/icons/hicolor/symbolic/apps/$(APP_ID)-symbolic.svg

# Run `recoolor restore` first: this removes the program, not what it applied.
uninstall:
	rm -f $(DESTDIR)$(PREFIX)/bin/recoolor \
		$(SHARE)/applications/$(APP_ID).desktop \
		$(SHARE)/metainfo/$(APP_ID).metainfo.xml \
		$(SHARE)/icons/hicolor/scalable/apps/$(APP_ID).svg \
		$(SHARE)/icons/hicolor/symbolic/apps/$(APP_ID)-symbolic.svg
