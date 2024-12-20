VERSION --global-cache 0.7

IMPORT github.com/earthly/lib/rust AS rust

install:
  FROM rust:1.82.0-bookworm
  RUN rustup component add clippy rustfmt

  # Call +INIT before copying the source file to avoid installing function depencies every time source code changes
  # This parametrization will be used in future calls to functions of the library
  DO rust+INIT --keep_fingerprints=true

source:
  FROM +install
  COPY --keep-ts Cargo.toml Cargo.lock ./
  COPY --keep-ts --dir bb-server ai-client common api_types bb-admin ./

# lint runs cargo clippy on the source code
lint:
  FROM +source
  # DO rust+CARGO --args="clippy --all-features --all-targets -- -D warnings"
  # DO rust+CARGO --args="clippy --all-features --all-targets"

build:
  FROM +lint
  DO rust+CARGO --args="build --release" --output="release/[^/\.]+"
  SAVE ARTIFACT ./target/release/ target AS LOCAL artifact/target

docker:
  ARG docker_tag=jerecan/blackballgame:blackballgame-server
  ARG ENVIRONMENT=production

  # FROM DOCKERFILE . # how to fix: https://docs.earthly.dev/docs/earthfile#description-10
  FROM debian:bookworm-slim # does work with libssl-dev
  RUN apt-get update && apt-get install -y libssl-dev
  WORKDIR /myapp
  COPY +build/target/bb-server /myapp
  EXPOSE 8080
  CMD ["./bb-server"]
  SAVE IMAGE --push "$docker_tag"

# test executes all unit and integration tests via Cargo
test:
  # LOCALLY
  FROM +lint
  DO rust+CARGO --args="test"

# fmt checks whether Rust code is formatted according to style guidelines
fmt:
  FROM +lint
  DO rust+CARGO --args="fmt --check"

# all runs all other targets in parallel
all:
  BUILD +build
  # BUILD +test
  # BUILD +fmt
