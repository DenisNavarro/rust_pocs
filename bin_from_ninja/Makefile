
.DELETE_ON_ERROR:
MAKEFLAGS += --no-builtin-rules
MAKEFLAGS += --warn-undefined-variables

cargo_target_dir != cargo metadata --format-version=1 | jq -r .target_directory

.PHONY: check # For each project, if not done yet, reformat the code and check it
check : build.ninja
	ninja check -v

#############################################
# Other phony targets in alphabetical order #
#############################################

.PHONY: all # For each project, if not done yet, reformat the code, check it and, if all is good, deploy it
all : build.ninja
	ninja -v

.PHONY: fmt # For each project, if not done yet, reformat the code
fmt : build.ninja
	ninja fmt -v

.PHONY: help # Print each phony target with its description
help:
	@grep '^.PHONY: .* # ' Makefile | sed 's/\.PHONY: \(.*\) # \(.*\)/\1\t\2/' | expand -t 6

################
# File targets #
################

Cargo.lock : Cargo.toml $(wildcard */Cargo.toml)
	cargo fetch && touch $@

fetch.maketarget : Cargo.lock
	cargo fetch && touch $@

$(cargo_target_dir)/debug/ninja_bootstrap : fetch.maketarget $(wildcard ninja_bootstrap/src/*.rs)
	cargo build --offline --frozen -p ninja_bootstrap && touch $@

build.ninja : $(cargo_target_dir)/debug/ninja_bootstrap
	RUST_LIB_BACKTRACE=1 $< > $@

example.ninja : build.ninja
	echo '# `build.ninja` is a file identical to the current one, except that:' > $@
	echo '# - `/home/denis` will be replaced with your home directory,' >> $@
	echo '# - the absolute path of the `target` directory will also be different and' >> $@
	echo '# - the 4 lines of the current comment are not there.' >> $@
	cat $< >> $@
