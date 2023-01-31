
project_name = structured_concurrency
debug_exe_path = target/debug/$(project_name)
release_exe_path = target/release/$(project_name)

.DELETE_ON_ERROR:

.PHONY: debug
#: Debug execution
debug : $(debug_exe_path) clippy.maketarget
	$<

#############################################
# Other phony targets in alphabetical order #
#############################################

.PHONY: clean
#: Remove what is in .gitignore
clean :
	git clean -dXf

.PHONY: debug2
#: Debug execution with >/dev/full (will panic)
debug2 : $(debug_exe_path) clippy.maketarget
	$< >/dev/full

.PHONY: edit
#: Edit the Makefile
edit :
	@codium Makefile

.PHONY: help
#: Print the help with remake from https://remake.readthedocs.io/
help :
	@remake --tasks

.PHONY: release
#: Release execution
release : $(release_exe_path) clippy.maketarget
	$<

.PHONY: release2
#: Release execution with >/dev/full (will panic)
release2 : $(release_exe_path) clippy.maketarget
	$< >/dev/full

################
# File targets #
################

clippy.maketarget : fmt.maketarget
	cargo clippy -- -D warnings && touch $@

#: Reformat the Rust files
fmt.maketarget : $(wildcard src/*.rs)
	cargo fmt && touch $@

$(debug_exe_path) : Cargo.toml fmt.maketarget
	cargo build

$(release_exe_path) : Cargo.toml fmt.maketarget
	cargo build --release
