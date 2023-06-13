
.DELETE_ON_ERROR:

.PHONY: check
#: For each project, if not done yet, reformat the code and check it
check : build.ninja
	ninja check -v

#############################################
# Other phony targets in alphabetical order #
#############################################

.PHONY: all
#: For each project, if not done yet, reformat the code, check it and, if all is good, deploy it
all : build.ninja
	ninja -v

.PHONY: clean
#: Remove what is in .gitignore (git clean -dXf)
clean :
	git clean -dXf

.PHONY: edit
#: Edit the Makefile (codium Makefile)
edit :
	@codium Makefile

.PHONY: fmt
#: For each project, if not done yet, reformat the code
fmt : build.ninja
	ninja fmt -v

.PHONY: help
#: Print the help with remake from https://remake.readthedocs.io/
help :
	@remake --tasks

.PHONY: watch
#: Reformat the code each time a Rust file is modified (thanks to the `watchexec-cli` crate)
watch : build.ninja
	watchexec --exts rs ninja fmt -v

################
# File targets #
################

#: Ensure that Cargo.lock matches the Cargo.toml files
Cargo.lock : Cargo.toml $(wildcard */Cargo.toml)
	cargo fetch && touch Cargo.lock

#: Fetch dependencies
fetch.maketarget : Cargo.lock
	cargo fetch && touch fetch.maketarget

#: Compile the Rust binary responsible to write the build.ninja file
target/debug/ninja_bootstrap : fetch.maketarget $(wildcard ninja_bootstrap/src/*.rs)
	cargo build --offline --frozen -p ninja_bootstrap && touch target/debug/ninja_bootstrap

#: Write the build.ninja file, almost identical to example.ninja
build.ninja : target/debug/ninja_bootstrap
	RUST_LIB_BACKTRACE=1 target/debug/ninja_bootstrap > build.ninja

#: Write example.ninja which is almost a copy of build.ninja, but is not in the .gitignore
example.ninja : build.ninja
	echo '# `build.ninja` is a file identical to the current one, except that:' > example.ninja
	echo '# - `/home/denis` will be replaced with your home directory and' >> example.ninja
	echo '# - the 3 lines of the current comment are not there.' >> example.ninja
	cat build.ninja >> example.ninja
