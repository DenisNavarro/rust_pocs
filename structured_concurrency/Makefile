
project_name = structured_concurrency
rust_files = $(wildcard src/*.rs)
maketarget_files = fmt.maketarget clippy.maketarget test.maketarget
debug_exe = target/debug/$(project_name)
release_exe = target/release/$(project_name)

.DELETE_ON_ERROR:
.NOTPARALLEL:

.PHONY: default
#: Fix code and launch debug execution
default :
	$(MAKE) fix && $(MAKE) debug

#############################################
# Other phony targets in alphabetical order #
#############################################

.PHONY: checks
#: rustfmt, clippy and unit tests
checks : $(maketarget_files)

.PHONY: clean
#: Remove what is in .gitignore
clean :
	@echo
	@echo '******************'
	@echo '* git clean -dXf *'
	@echo '******************'
	@echo
	@git clean -dXf

.PHONY: debug
#: Debug execution
debug : $(maketarget_files) $(debug_exe)
	@echo
	@echo '*******************'
	@echo '* Debug execution *'
	@echo '*******************'
	@echo
	$(debug_exe)

.PHONY: debug2
#: Debug execution with >/dev/full
debug2 : $(maketarget_files) $(debug_exe)
	@echo
	@echo '***********************************'
	@echo '* Debug execution with >/dev/full *'
	@echo '***********************************'
	@echo
	$(debug_exe) >/dev/full

.PHONY: doc
#: cargo doc --open
doc :
	@echo
	@echo '********************'
	@echo '* cargo doc --open *'
	@echo '********************'
	@echo
	@cargo doc --open

.PHONY: edit
#: Edit the Makefile
edit :
	@codium Makefile

.PHONY: fix
#: Fix with rustfmt and touch fmt.maketarget
fix : $(rust_files)
	@echo
	@echo '************'
	@echo '* Fix code *'
	@echo '************'
	@echo
	@for file_path in $^; do \
	    if [ ! -f fmt.maketarget -o $$file_path -nt fmt.maketarget ]; then \
	        echo rustfmt -- $$file_path; \
	        rustfmt -- $$file_path; \
	    fi \
	done
	touch fmt.maketarget

.PHONY: help
help :
	@remake --tasks

.PHONY: release
#: Release execution
release : $(maketarget_files) $(release_exe)
	@echo
	@echo '*********************'
	@echo '* Release execution *'
	@echo '*********************'
	@echo
	$(release_exe)

.PHONY: release2
#: Release execution with >/dev/full
release2 : $(maketarget_files) $(release_exe)
	@echo
	@echo '*************************************'
	@echo '* Release execution with >/dev/full *'
	@echo '*************************************'
	@echo
	$(release_exe) >/dev/full

.PHONY: verify
#: Clean and launch debug execution
verify :
	$(MAKE) clean && $(MAKE) debug

################
# File targets #
################

clippy.maketarget : $(rust_files)
	@echo
	@echo '**********'
	@echo '* Clippy *'
	@echo '**********'
	@echo
	cargo clippy -- -D warnings
	touch $@

fmt.maketarget : $(rust_files)
	@echo
	@echo '****************'
	@echo '* Format check *'
	@echo '****************'
	@echo
	@for file_path in $?; do \
	    echo rustfmt --check -- $$file_path; \
	    rustfmt --check -- $$file_path; \
	done
	touch $@

test.maketarget : $(rust_files) Cargo.toml
	@echo
	@echo '**************'
	@echo '* Unit tests *'
	@echo '**************'
	@echo
	cargo test
	touch $@

$(debug_exe) : $(rust_files) Cargo.toml
	@echo
	@echo '***************'
	@echo '* Debug build *'
	@echo '***************'
	@echo
	cargo build

$(release_exe) : $(rust_files) Cargo.toml
	@echo
	@echo '*****************'
	@echo '* Release build *'
	@echo '*****************'
	@echo
	cargo build --release
