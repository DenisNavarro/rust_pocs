.DELETE_ON_ERROR:
MAKEFLAGS += --no-builtin-rules
MAKEFLAGS += --warn-undefined-variables

.PHONY: help # Print each phony target with its description
help:
	@grep '^.PHONY: .* # ' Makefile | sed 's/\.PHONY: \(.*\) # \(.*\)/\1\t\2/' | expand -t 24

#############################################
# Other phony targets in alphabetical order #
#############################################

.PHONY: clean # Remove what is in `.gitignore`
clean :
	git clean -dXf

.PHONY: edit # Edit the `Makefile`
edit :
	@codium Makefile

.PHONY: install_git_hooks # Install Git hooks
install_git_hooks:
	cp pre-commit.sh .git/hooks/pre-commit

.PHONY: install_rust_toolchains # Install the Rust toolchains used by the Git hooks
install_rust_toolchains:
	rustup toolchain install 1.81.0 --profile minimal --component clippy,rustfmt
