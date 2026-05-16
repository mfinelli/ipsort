PREFIX := /usr/local
DESTDIR :=

GREP := grep
ifeq ($(shell uname), Darwin)
	GREP := ggrep
endif

VERSION ?= $(shell $(GREP) -m1 '^version ' Cargo.toml | awk -F\" '{print $$2}')
TODAY ?= $(shell date +%Y-%m-%d)

all: target/release/ipsort target/release/ipsort.1 target/release/ipsort.bash \
	target/release/ipsort.fish target/release/ipsort.zsh

clean:
	rm -rf target

target/release/ipsort: $(wildcard src/*.rs)
	cargo build --frozen --release

target/release/ipsort.bash: target/release/ipsort
	./target/release/ipsort --generate-completions bash > $@

target/release/ipsort.fish: target/release/ipsort
	./target/release/ipsort --generate-completions fish > $@

target/release/ipsort.zsh: target/release/ipsort
	./target/release/ipsort --generate-completions zsh > $@

target/release/ipsort.1: ipsort.1.scd
	sed -e "s/__VERSION__/$(VERSION)/" -e "s/__DATE__/$(TODAY)/" \
		$< | scdoc > $@

install: all
	install -Dm0755 target/release/ipsort "$(DESTDIR)$(PREFIX)/bin/ipsort"
	install -Dm0644 README.md \
		"$(DESTDIR)$(PREFIX)/share/doc/ipsort/README.md"
	install -Dm0644 target/release/ipsort/ipsort.bash \
		"$(DESTDIR)$(PREFIX)/usr/share/bash-completion/completions/ipsort"
	install -Dm0644 target/release/ipsort/ipsort.fish \
		"$(DESTDIR)$(PREFIX)/usr/share/fish/vendor_completions.d/ipsort.fish"
	install -Dm0644 target/release/ipsort/ipsort.zsh \
		"$(DESTDIR)$(PREFIX)/usr/share/zsh/site-functions/_ipsort"
	install -Dm0644 target/releases/ipsort.1 \
		"$(DESTDIR)$(PREFIX)/usr/share/man/man1/ipsort.1"

uninstall:
	rm -rf \
		"$(DESTDIR)$(PREFIX)/bin/ipsort" \
		"$(DESTDIR)$(PREFIX)/share/doc/ipsort" \
		"$(DESTDIR)$(PREFIX)/share/bash-completion/completions/completions/ipsort" \
		"$(DESTDIR)$(PREFIX)/share/fish/vendor_completions.d/ipsort.fish" \
		"$(DESTDIR)$(PREFIX)/share/doc/zsh/site-functions/_ipsort" \
		"$(DESTDIR)$(PREFIX)/share/man/man1/ipsort.1"

.PHONY: all clean install uninstall
