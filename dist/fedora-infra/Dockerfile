# build environment
FROM fedora:30 AS build-env

# build: system utilities and libraries
RUN dnf -y install g++ openssl-devel

# build: pinned Rust stable toolchain
ADD https://static.rust-lang.org/dist/rust-1.34.2-x86_64-unknown-linux-gnu.tar.gz rust.tar.gz
RUN tar -xf rust.tar.gz --strip 1
RUN ./install.sh

# build: release binary
ADD . /src
RUN cd /src && cargo build --release

# run environment
FROM fedora:30
WORKDIR /
COPY --from=build-env /src/target/release/dumnati /usr/local/bin/dumnati
CMD [ "/usr/local/bin/dumnati" ]
