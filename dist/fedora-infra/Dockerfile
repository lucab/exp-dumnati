FROM fedora:30

# build: system utilities and libraries
RUN dnf -y install g++ openssl-devel

# build: pinned Rust stable toolchain
#ADD https://static.rust-lang.org/dist/rust-1.34.2-x86_64-unknown-linux-gnu.tar.gz rust.tar.gz
#RUN tar -xf rust.tar.gz --strip 1 && ./install.sh

# build: system Rust toolchain
RUN dnf -y install rust cargo

# build: release binary
ADD . /src
RUN cd /src && cargo build --release && mv /src/target/release/dumnati /usr/local/bin/dumnati

# build: cleanup
RUN cd /src && cargo clean

# run: default config
WORKDIR /
CMD [ "/usr/local/bin/dumnati" ]
