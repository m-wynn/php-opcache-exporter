FROM ekidd/rust-musl-builder:latest AS build
COPY ./src ./src
COPY Cargo.toml Cargo.lock ./
RUN cargo build --release

FROM alpine:latest as certs
RUN apk --update add ca-certificates

FROM scratch
COPY --from=certs /etc/ssl/certs/ca-certificates.crt /etc/ssl/certs/ca-certificates.crt
COPY --from=build /home/rust/src/target/x86_64-unknown-linux-musl/release/ci-helper-service /
COPY testers.toml /
USER 1000
ENV RUST_LOG WARN
CMD ["/php-opcache-exporter-rs"]
EXPOSE 8080/tcp
