.DEFAULT_GOAL := all

CARGO ?= cargo
CARGO_FMT ?= cargo fmt
CARGO_CLIPPY ?= cargo clippy
CARGO_LLVM_COV ?= cargo llvm-cov
DOXYGEN ?= doxygen
SHELLCHECK ?= shellcheck
CPPCHECK ?= cppcheck
READELF ?= readelf
CC ?= cc
PYTHON ?= python3
PKG_CONFIG ?= pkg-config

PACKAGE := librptadvschedule
CRATE := rptadvschedule
PACKAGE_VERSION ?= 0.1.0-alpha.3
SOVERSION := 1
PREFIX ?= /usr/local
DESTDIR ?=
LIBDIR ?= $(PREFIX)/lib
CARGO_TARGET_DIR ?= target
TARGET_RELEASE := $(CARGO_TARGET_DIR)/release
LIBRARY_BASENAME := lib$(CRATE)
TARGET_LIBRARY := $(TARGET_RELEASE)/$(LIBRARY_BASENAME).so
LIBRARY_VERSIONED := build/$(LIBRARY_BASENAME).so.$(SOVERSION).$(PACKAGE_VERSION)
LIBRARY_SONAME := build/$(LIBRARY_BASENAME).so.$(SOVERSION)
LIBRARY_LINK := build/$(LIBRARY_BASENAME).so
HEADERS := $(wildcard include/rptadvschedule/*.h)
RUST_SOURCES := $(wildcard src/*.rs src/*/*.rs third_party/hron/src/*.rs third_party/hron/src/eval/*.rs)
PC_TEMPLATE := rptadvschedule.pc.in
PC_FILE := build/rptadvschedule.pc
C_SMOKE_SOURCES := tests/descriptor_smoke.c tests/recurrence_smoke.c
C_SMOKE_BINARIES := $(patsubst tests/%.c,build/%,$(C_SMOKE_SOURCES))
DEBIAN_VERSION = $(shell dpkg-parsechangelog -S Version)
DEBIAN_ARCH = $(shell dpkg-architecture -qDEB_HOST_ARCH)
DEBIAN_MULTIARCH = $(shell dpkg-architecture -qDEB_HOST_MULTIARCH)
DEBIAN_SOURCE_PARENT = build/debian-source
DEBIAN_OUTPUT_DIR = $(abspath $(DEBIAN_SOURCE_PARENT))
DEBIAN_RUNTIME_DEB = $(DEBIAN_OUTPUT_DIR)/librptadvschedule$(SOVERSION)_$(DEBIAN_VERSION)_$(DEBIAN_ARCH).deb
DEBIAN_DEV_DEB = $(DEBIAN_OUTPUT_DIR)/librptadvschedule-dev_$(DEBIAN_VERSION)_$(DEBIAN_ARCH).deb
DEBIAN_STAGE = build/debian-package-stage
COVERAGE_TOOLCHAIN ?= nightly-2026-01-25
COVERAGE_DIR = build/coverage
COVERAGE_TARGET_DIR = build/llvm-cov-target
COVERAGE_JSON = $(COVERAGE_DIR)/coverage.json
COVERAGE_ENV = RUSTUP_TOOLCHAIN=$(COVERAGE_TOOLCHAIN) CARGO_LLVM_COV_TARGET_DIR=$(abspath $(COVERAGE_TARGET_DIR))
QUALITY_BASE_IMAGE ?= ghcr.io/cpeter1207/rpt-advanced-quality-debian13:latest
QUALITY_IMAGE ?= $(PACKAGE)-quality:local
QUALITY_LAUNCHER = tools/run-in-quality-container.sh

# Rust emits the ABI-major SONAME while this Makefile creates conventional
# versioned and linker symlinks for Debian and pkg-config consumers.
SONAME_RUSTFLAGS = $(RUSTFLAGS) -C link-arg=-Wl,-soname,$(LIBRARY_BASENAME).so.$(SOVERSION)

.PHONY: all quality lint static-analysis docs rustdoc test coverage install install-check \
	debian-package-check dist distcheck platform-verify ci quality-image container-coverage clean FORCE

all: $(LIBRARY_VERSIONED) $(LIBRARY_SONAME) $(LIBRARY_LINK)

build:
	mkdir -p $@

$(TARGET_LIBRARY): Makefile Cargo.toml Cargo.lock $(RUST_SOURCES)
	RUSTFLAGS="$(SONAME_RUSTFLAGS)" $(CARGO) build --release --locked

$(LIBRARY_VERSIONED): $(TARGET_LIBRARY) | build
	cp $(TARGET_LIBRARY) $@
	$(READELF) -d $@ | grep -F '$(LIBRARY_BASENAME).so.$(SOVERSION)'
	! $(READELF) -d $@ | grep -E 'libstd-|\((RPATH|RUNPATH)\)'

$(LIBRARY_SONAME): $(LIBRARY_VERSIONED)
	ln -sf $(notdir $<) $@

$(LIBRARY_LINK): $(LIBRARY_SONAME)
	ln -sf $(notdir $<) $@

$(PC_FILE): $(PC_TEMPLATE) FORCE | build
	sed -e 's|@PREFIX@|$(PREFIX)|' -e 's|@LIBDIR@|$(LIBDIR)|' \
		-e 's|@VERSION@|$(PACKAGE_VERSION)|' -e 's|@ABI_VERSION@|$(SOVERSION)|' $< > $@

quality: lint static-analysis docs

lint:
	$(PYTHON) tests/coverage_audit.py
	$(CARGO_FMT) --check
	rustfmt --check --edition 2021 third_party/hron/src/eval/captured.rs
	$(SHELLCHECK) tools/run-in-quality-container.sh

static-analysis:
	$(CARGO_CLIPPY) --all-targets --all-features -- -D warnings
	$(CARGO_CLIPPY) -p hron --lib --locked -- -D warnings
	$(CPPCHECK) --force --enable=warning,style,performance,portability \
		--error-exitcode=1 --std=c11 -Iinclude $(C_SMOKE_SOURCES)

docs: rustdoc | build
	$(DOXYGEN) Doxyfile
	test ! -s build/doxygen-warnings.log

rustdoc:
	RUSTDOCFLAGS="-D warnings" $(CARGO) doc --no-deps --locked --document-private-items

test:
	$(CARGO) test --all-targets --locked
	$(MAKE) $(C_SMOKE_BINARIES)
	set -e; for smoke in $(C_SMOKE_BINARIES); do ./$$smoke; done

$(C_SMOKE_BINARIES): build/%: tests/%.c $(HEADERS) $(LIBRARY_LINK) | build
	$(CC) -std=c11 -Wall -Wextra -Werror -Iinclude $< -Lbuild \
		-l$(CRATE) -Wl,-rpath,'$$ORIGIN' -o $@

# Native Debian 13 amd64 only: audit the JSON rather than trusting a human
# summary, and exclude the in-tree Rust test module from production coverage.
coverage:
	rm -rf $(COVERAGE_DIR) $(COVERAGE_TARGET_DIR)
	mkdir -p $(COVERAGE_DIR)
	$(COVERAGE_ENV) $(CARGO_LLVM_COV) --all-targets --locked --branch --json --output-path $(COVERAGE_JSON)
	$(COVERAGE_ENV) $(CARGO_LLVM_COV) report --lcov --output-path $(COVERAGE_DIR)/coverage.lcov
	$(PYTHON) tools/check-coverage.py $(COVERAGE_DIR)

quality-image:
	docker image pull $(QUALITY_BASE_IMAGE)
	docker build --pull --build-arg BASE_IMAGE=$(QUALITY_BASE_IMAGE) --tag $(QUALITY_IMAGE) \
		--file containers/quality.Dockerfile containers

container-coverage: quality-image
	RPTADV_CONTAINER_PULL=0 sh $(QUALITY_LAUNCHER) $(QUALITY_IMAGE) $(MAKE) coverage

install: all $(PC_FILE)
	install -d $(DESTDIR)$(LIBDIR) $(DESTDIR)$(PREFIX)/include/rptadvschedule \
		$(DESTDIR)$(LIBDIR)/pkgconfig
	install -m 0755 $(LIBRARY_VERSIONED) $(DESTDIR)$(LIBDIR)/
	ln -sf $(notdir $(LIBRARY_VERSIONED)) $(DESTDIR)$(LIBDIR)/$(notdir $(LIBRARY_SONAME))
	ln -sf $(notdir $(LIBRARY_SONAME)) $(DESTDIR)$(LIBDIR)/$(notdir $(LIBRARY_LINK))
	install -m 0644 $(HEADERS) $(DESTDIR)$(PREFIX)/include/rptadvschedule/
	install -m 0644 $(PC_FILE) $(DESTDIR)$(LIBDIR)/pkgconfig/

install-check: all
	rm -rf build/stage
	$(MAKE) DESTDIR=$(CURDIR)/build/stage PREFIX=/usr LIBDIR=/usr/lib install
	test -f build/stage/usr/lib/$(notdir $(LIBRARY_VERSIONED))
	test -L build/stage/usr/lib/$(notdir $(LIBRARY_SONAME))
	test -L build/stage/usr/lib/$(notdir $(LIBRARY_LINK))
	test ! -e build/stage/usr/lib/$(LIBRARY_BASENAME).a
	$(READELF) -d build/stage/usr/lib/$(notdir $(LIBRARY_VERSIONED)) | grep -F '$(LIBRARY_BASENAME).so.$(SOVERSION)'
	! $(READELF) -d build/stage/usr/lib/$(notdir $(LIBRARY_VERSIONED)) | grep -E 'res_usbradio.so|libportaudio.so|libasound.so|libhamlib.so|libavfilter.so'
	set -e; for header in $(notdir $(HEADERS)); do test -f build/stage/usr/include/rptadvschedule/$$header; done
	test -f build/stage/usr/lib/pkgconfig/rptadvschedule.pc
	test "$$(PKG_CONFIG_PATH=$(CURDIR)/build/stage/usr/lib/pkgconfig $(PKG_CONFIG) --variable=abi_version rptadvschedule)" = '$(SOVERSION)'
	test "$$(PKG_CONFIG_PATH=$(CURDIR)/build/stage/usr/lib/pkgconfig $(PKG_CONFIG) --variable=recurrence_abi_version rptadvschedule)" = '1'
	$(MAKE) staged-smoke STAGE=$(CURDIR)/build/stage STAGE_LIBDIR=/usr/lib

.PHONY: staged-smoke
staged-smoke:
	export PKG_CONFIG_PATH="$(STAGE)$(STAGE_LIBDIR)/pkgconfig" PKG_CONFIG_SYSROOT_DIR="$(STAGE)"; \
	set -e; for source in $(C_SMOKE_SOURCES); do \
		binary="$(STAGE)/$$(basename "$$source" .c)"; \
		$(CC) -std=c11 -Wall -Wextra -Werror "$$source" $$($(PKG_CONFIG) --cflags --libs rptadvschedule) -o "$$binary"; \
		LD_LIBRARY_PATH="$(STAGE)$(STAGE_LIBDIR):$$LD_LIBRARY_PATH" "$$binary"; \
	done

debian-package-check: dist
	rm -rf $(DEBIAN_SOURCE_PARENT) $(DEBIAN_STAGE)
	mkdir -p $(DEBIAN_SOURCE_PARENT)
	build_root=$$(mktemp -d); \
	trap 'rm -rf "$$build_root"' EXIT; \
	tar -C "$$build_root" -xzf build/$(PACKAGE)-$(PACKAGE_VERSION).tar.gz; \
	source_dir="$$build_root/$(PACKAGE)-$(PACKAGE_VERSION)"; \
	chmod 0644 "$$source_dir"/debian/changelog "$$source_dir"/debian/control \
		"$$source_dir"/debian/copyright "$$source_dir"/debian/*.docs \
		"$$source_dir"/debian/*.install "$$source_dir"/debian/source/*; \
	chmod 0755 "$$source_dir"/debian/rules; \
	cd "$$source_dir" && dpkg-buildpackage -us -uc -b; \
	cp "$$build_root"/*.deb "$(DEBIAN_OUTPUT_DIR)/"
	test -f "$(DEBIAN_RUNTIME_DEB)"
	test -f "$(DEBIAN_DEV_DEB)"
	rm -rf $(DEBIAN_STAGE)
	mkdir -p $(DEBIAN_STAGE)
	dpkg-deb --extract "$(DEBIAN_RUNTIME_DEB)" $(DEBIAN_STAGE)
	dpkg-deb --extract "$(DEBIAN_DEV_DEB)" $(DEBIAN_STAGE)
	test ! -e "$(DEBIAN_STAGE)/usr/lib/$(DEBIAN_MULTIARCH)/$(LIBRARY_BASENAME).a"
	test -L "$(DEBIAN_STAGE)/usr/lib/$(DEBIAN_MULTIARCH)/$(LIBRARY_BASENAME).so"
	$(READELF) -d "$(DEBIAN_STAGE)/usr/lib/$(DEBIAN_MULTIARCH)/$(LIBRARY_BASENAME).so.$(SOVERSION)" | grep -F '$(LIBRARY_BASENAME).so.$(SOVERSION)'
	! $(READELF) -d "$(DEBIAN_STAGE)/usr/lib/$(DEBIAN_MULTIARCH)/$(LIBRARY_BASENAME).so.$(SOVERSION)" | grep -E 'res_usbradio.so|libportaudio.so|libasound.so|libhamlib.so|libavfilter.so'
	$(MAKE) staged-smoke STAGE=$(CURDIR)/$(DEBIAN_STAGE) STAGE_LIBDIR=/usr/lib/$(DEBIAN_MULTIARCH)

dist: | build
	rm -rf build/dist
	mkdir -p build/dist/$(PACKAGE)-$(PACKAGE_VERSION)
	tar --exclude=.git --exclude=.work --exclude=build --exclude=target --exclude=__pycache__ \
		--exclude=debian/.debhelper --exclude=debian/debhelper-build-stamp \
		--exclude=debian/files --exclude=debian/tmp \
		--exclude=debian/librptadvschedule1 --exclude=debian/librptadvschedule-dev \
		--exclude='debian/*.substvars' --exclude='debian/*.debhelper.log' \
		--transform='s|^|$(PACKAGE)-$(PACKAGE_VERSION)/|' -czf build/$(PACKAGE)-$(PACKAGE_VERSION).tar.gz \
		.dockerignore AGENTS.md Cargo.lock Cargo.toml COPYING Doxyfile Makefile QUALITY.md README.md \
		rptadvschedule.pc.in rust-toolchain.toml containers debian include src tests third_party tools

distcheck: dist
	! tar -tzf build/$(PACKAGE)-$(PACKAGE_VERSION).tar.gz | \
		grep -E '/debian/(\.debhelper/|debhelper-build-stamp$$|files$$|tmp/|librptadvschedule([0-9]+|-dev)/|.*\.(substvars|debhelper\.log)$$)'
	rm -rf build/dist-unpacked
	mkdir -p build/dist-unpacked
	tar -C build/dist-unpacked -xzf build/$(PACKAGE)-$(PACKAGE_VERSION).tar.gz
	$(MAKE) -C build/dist-unpacked/$(PACKAGE)-$(PACKAGE_VERSION) install-check

platform-verify: test install-check debian-package-check distcheck

ci: quality platform-verify

clean:
	rm -rf build target

FORCE:
