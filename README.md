
# Lurk

![Build & Test](https://github.com/boris-sinyapkin/lurk/actions/workflows/build-and-test.yaml/badge.svg?branch=master)

## Overview

**Lurk** is an async and lightweight implementation of a SOCKS5 proxy, allowing users to establish the connection through intermediate data relaying server. It's entirely built in Rust with [tokio-async](https://tokio.rs) runtime.

## Getting Started

### Prerequisites

- For dev purposes: rust toolchain. Visit [Rust's official page](https://www.rust-lang.org/) for installation details.
- [Docker](https://www.docker.com) for running out-of-the-box.

## Run Docker container

Lurk proxy could be deployed by using docker image stored in GitHub Package's.

Publish port to default 1080 and start listening incoming connections:

Run [**latest release**](https://github.com/boris-sinyapkin/lurk/releases/latest) in the Docker container:

```bash
docker run --rm --name lurk -p 1080:1080/tcp ghcr.io/boris-sinyapkin/lurk:latest
```

Run [**latest nightly build**](https://github.com/boris-sinyapkin/lurk/pkgs/container/lurk-nightly) in the Docker container:

```bash
docker run --rm --name lurk-nightly -p 1080:1080/tcp ghcr.io/boris-sinyapkin/lurk-nightly:latest
```

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

## Run benchmark tool against Lurk

Lurk server can be stressed by some HTTP benchmark, e.g. [rsb project](https://github.com/gamelife1314/rsb).

Below bash script will deploy proxy server and stress it by running this benchmark tool.
As [rsb project](https://github.com/gamelife1314/rsb) uses [request](https://github.com/gamelife1314/rsb?tab=readme-ov-file#proxy) create,
it's possible to test proxied connections.

```bash
# Create bridge docker network
docker network create lurk-network

# Run Lurk in docker container and attach it to created bridge lurk-network
docker run --rm --network lurk-network --name lurk-server -p 1080:1080/tcp ghcr.io/boris-sinyapkin/lurk:latest
```

Wait until it deploys and run benchmark in a separate shell:

```bash
# Run benchmark with 1000 HTTP GET requests perfomed over 100 connections
docker run --rm --network lurk-network --name rsb-benchmark -e http_proxy=socks5://lurk-server:1080 \
  ghcr.io/gamelife1314/rsb:latest --requests 1000 --connections 100 -l --timeout 5 http://example.com

# Clean-up
docker kill lurk-server
docker network rm lurk-network
```
