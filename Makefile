PREFIX := /usr/local
DESTDIR :=

all: ipsort

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

install: target/release/ipsort target/release/ipsort.bash \
	target/release/ipsort.fish target/release/ipsort.zsh
	install -Dm0755 target/release/ipsort "$(DESTDIR)$(PREFIX)/bin/ipsort"
	install -Dm0644 README.md \
		"$(DESTDIR)$(PREFIX)/share/doc/ipsort/README.md"
	install -Dm0644 tartet/release/ipsort/ipsort.bash \
		"$(DESTDIR)$(PREFIX)/usr/share/bash-completion/completions/ipsort"
	install -Dm0644 tartet/release/ipsort/ipsort.fish \
		"$(DESTDIR)$(PREFIX)/usr/share/fish/vendor_completions.d/ipsort.fish"
	install -Dm0644 tartet/release/ipsort/ipsort.zsh \
		"$(DESTDIR)$(PREFIX)/usr/share/zsh/site-functions/_ipsort"

uninstall:
	rm -rf \
		"$(DESTDIR)$(PREFIX)/bin/ipsort" \
		"$(DESTDIR)$(PREFIX)/share/doc/ipsort" \
		"$(DESTDIR)$(PREFIX)/share/bash-completion/completions/completions/ipsort" \
		"$(DESTDIR)$(PREFIX)/share/fish/vendor_completions.d/ipsort.fish" \
		"$(DESTDIR)$(PREFIX)/share/doc/zsh/site-functions/_ipsort"

.PHONY: all clean install uninstall
