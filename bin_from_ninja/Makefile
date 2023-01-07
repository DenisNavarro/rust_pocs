
ninja_bootstrap_code = ninja_bootstrap/Cargo.toml $(wildcard ninja_bootstrap/src/*.rs)
ninja_bootstrap = target/debug/ninja_bootstrap

.DELETE_ON_ERROR:

.PHONY: check
#: Reformat the Rust files and check
check : build.ninja
	ninja check -v

.PHONY: fmt
#: Reformat the Rust files
fmt : build.ninja
	ninja fmt -v

.PHONY: all
#: Reformat the Rust files, check and deploy
all : build.ninja
	ninja -v

build.ninja : Cargo.toml $(ninja_bootstrap_code) | $(ninja_bootstrap)
	RUST_LIB_BACKTRACE=1 $| > $@

$(ninja_bootstrap) : $(ninja_bootstrap_code)
	cargo build -p ninja_bootstrap

.PHONY: clean
#: Remove what is in .gitignore (git clean -dXf)
clean :
	git clean -dXf

.PHONY: edit
#: Edit the Makefile (codium Makefile)
edit :
	@codium Makefile

.PHONY: help
help :
	@remake --tasks
