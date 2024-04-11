
![GitHub CI dev](https://github.com/boris-sinyapkin/lurk/actions/workflows/ci.yaml/badge.svg?branch=dev)


# Lurk

**Lurk** is an async and lightweight implementation of a SOCKS5 proxy, allowing users to establish the connection through intermediate data relaying server. It's entirely built in Rust with [tokio-async](https://tokio.rs) runtime.

## Getting Started

### Prerequisites

- For dev purposes: rust toolchain. Visit [Rust's official page](https://www.rust-lang.org/) for installation details.
- [Docker](https://www.docker.com) for running out-of-the-box.

## Build & deploy from GitHub sources

Project could be compiled directly from sources:
```bash
git clone git@github.com:boris-sinyapkin/lurk.git
cd lurk
cargo build --release 
```

If you want to install binary to the /bin directory, run the following command from cloned repository:
```bash
cargo install --path .
```

By default, **Lurk** is listening on conventionally defined 1080 port (see [RFC 1928](https://datatracker.ietf.org/doc/html/rfc1928)):
```bash
Fast and fancy SOCKS5 proxy

Usage: lurk [OPTIONS]

Options:
  -p, --port <PORT>  TCP port to listen on [default: 1080]
  -i, --ipv4 <IPV4>  IPv4 to listen on [default: 127.0.0.1]
  -h, --help         Print help
  -V, --version      Print version
```

If default settings is acceptable, execute installed binary:
```bash
lurk
```
Either deploy server through **cargo**:
```bash
cargo run --release
```
