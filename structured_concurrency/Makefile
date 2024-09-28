
.DELETE_ON_ERROR:
MAKEFLAGS += --no-builtin-rules
MAKEFLAGS += --warn-undefined-variables

.PHONY: debug # Debug execution
debug : clippy.maketarget
	cargo run

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
release : clippy.maketarget
	cargo run -r

################
# File targets #
################

fmt.maketarget : $(wildcard src/*.rs)
	cargo fmt && touch $@

clippy.maketarget : fmt.maketarget Cargo.toml
	cargo clippy --all-targets --all-features -- -D warnings && touch $@
