
.DELETE_ON_ERROR:
MAKEFLAGS += --no-builtin-rules
MAKEFLAGS += --warn-undefined-variables

.PHONY: debug # Debug execution
debug : test.maketarget
	echo ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz > app.log
	cargo run -q --bin basic_renamer -- app.log
	echo ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz > app.log
	cargo run -q --bin generic_renamer -- app.log
	echo ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz > app.log
	cargo run -q --bin async_renamer -- app.log
	echo ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz > app.log
	cargo run -q --bin sync_renamer -- app.log
	rm -f app.log.*

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
release : test.maketarget
	echo ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz > app.log
	cargo run -qr --bin basic_renamer -- app.log
	echo ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz > app.log
	cargo run -qr --bin generic_renamer -- app.log
	echo ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz > app.log
	cargo run -qr --bin async_renamer -- app.log
	echo ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz > app.log
	cargo run -qr --bin sync_renamer -- app.log
	rm -f app.log.*

################
# File targets #
################

fmt.maketarget : rustfmt.toml $(wildcard */*.rs)
	cargo fmt && touch $@

clippy.maketarget : fmt.maketarget Cargo.toml $(wildcard */Cargo.toml)
	cargo clippy --all-targets --all-features -- -D warnings && touch $@

test.maketarget : clippy.maketarget Cargo.lock
	cargo test && touch $@
