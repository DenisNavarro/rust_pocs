FROM docker.io/library/rust:1.97.1-slim-bookworm

ENV HOME="/root"

RUN set -eux; \
    rustup component add clippy rustfmt; \
    cargo install pixi --git https://github.com/prefix-dev/pixi.git --tag v0.73.0 --locked; \
    # Remark: `synchronize_backup` and `synchronize_partially` call `rsync`.
    pixi global install rsync=3.4.4; \
    rm -rf ~/.cargo/git/db ~/.cargo/git/checkouts ~/.cargo/registry/src \
      ~/.cargo/registry/cache ~/.cargo/registry/index; \
    pixi clean cache --yes

ENV PATH="$PATH:$HOME/.pixi/bin"

WORKDIR /work

COPY pixi.toml pixi.lock /work/

RUN set -eux; \
    pixi install; \
    pixi clean cache --yes

COPY . /work/

RUN set -eux; \
    pixi run all; \
    rm -rf ~/.cargo/git/db ~/.cargo/git/checkouts ~/.cargo/registry/src \
      ~/.cargo/registry/cache ~/.cargo/registry/index; \
    pixi clean cache --yes; \
    rm -rf .pixi target */clippy.ninjatarget */fmt.ninjatarget */test.ninjatarget .ninja_deps \
      .ninja_log build.ninja

# Make available `synchronize_backup`, `synchronize_partially`, etc.
ENV PATH="$PATH:$HOME/bin"

CMD ["/bin/bash"]
