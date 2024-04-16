FROM rust:latest AS builder

RUN adduser lurkrunner
USER lurkrunner

WORKDIR /lurk
ADD . .

# Install release build
RUN cargo install --path .

ENTRYPOINT [ "bash", "./docker/entrypoint.sh" ]