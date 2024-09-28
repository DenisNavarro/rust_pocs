.DELETE_ON_ERROR:
MAKEFLAGS += --no-builtin-rules
MAKEFLAGS += --warn-undefined-variables

.PHONY: help # Print each phony target with its description
help:
	@grep '^.PHONY: .* # ' Makefile | sed 's/\.PHONY: \(.*\) # \(.*\)/\1\t\2/' | expand -t 23

#############################################
# Other phony targets in alphabetical order #
#############################################

.PHONY: clean # Remove what is in `.gitignore`
clean :
	git clean -dXf

.PHONY: edit # Edit the `Makefile`
edit :
	@codium Makefile

.PHONY: install_git_hooks # Install Git hooks with Cocogitto
install_git_hooks:
	cog install-hook --all

.PHONY: install_rust_toolchain # Install the Rust toolchain used by the `pre-commit` hook
install_rust_toolchain:
	rustup toolchain install 1.81.0 --profile minimal --component clippy,rustfmt
