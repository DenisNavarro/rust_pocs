
.DELETE_ON_ERROR:
MAKEFLAGS += --no-builtin-rules
MAKEFLAGS += --warn-undefined-variables

debug_exe_path = target/debug/structured_concurrency
release_exe_path = target/release/structured_concurrency

.PHONY: debug # Debug execution
debug : $(debug_exe_path) clippy.maketarget
	$<

#############################################
# Other phony targets in alphabetical order #
#############################################

.PHONY: clean # Remove what is in .gitignore
clean :
	git clean -dXf

.PHONY: edit # Edit the Makefile
edit :
	@codium Makefile

.PHONY: help # Print each phony target with its description
help:
	@grep '^.PHONY: .* # ' Makefile | sed 's/\.PHONY: \(.*\) # \(.*\)/\1\t\2/' | expand -t 8

.PHONY: release # Release execution
release : $(release_exe_path) clippy.maketarget
	$<

################
# File targets #
################

fmt.maketarget : $(wildcard src/*.rs)
	cargo fmt && touch $@

clippy.maketarget : fmt.maketarget
	cargo clippy -- -D warnings && touch $@

$(debug_exe_path) : Cargo.toml fmt.maketarget
	cargo build

$(release_exe_path) : Cargo.toml fmt.maketarget
	cargo build --release
