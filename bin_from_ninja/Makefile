
.DELETE_ON_ERROR:

.PHONY: check
#: For each project, if not done yet, reformat the code and check it
check : build.ninja
	ninja check -v

.PHONY: fmt
#: For each project, if not done yet, reformat the code
fmt : build.ninja
	ninja fmt -v

.PHONY: all
#: For each project, if not done yet, reformat the code, check it and, if all is good, deploy it
all : build.ninja
	ninja -v

#: Write the build.ninja file, almost identical to example.ninja
build.ninja : target/debug/ninja_bootstrap Cargo.toml $(wildcard */Cargo.toml)
	RUST_LIB_BACKTRACE=1 target/debug/ninja_bootstrap > build.ninja

#: Compile the Rust binary responsible to write the build.ninja file
target/debug/ninja_bootstrap : ninja_bootstrap/Cargo.toml $(wildcard ninja_bootstrap/src/*.rs)
	cargo build -p ninja_bootstrap && touch target/debug/ninja_bootstrap

.PHONY: clean
#: Remove what is in .gitignore (git clean -dXf)
clean :
	git clean -dXf

.PHONY: edit
#: Edit the Makefile (codium Makefile)
edit :
	@codium Makefile

.PHONY: help
#: Print the help with remake from https://remake.readthedocs.io/
help :
	@remake --tasks
