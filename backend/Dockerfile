FROM rust:slim-bookworm AS builder

WORKDIR /build

# Copy the whole workspace so Cargo resolves deps correctly. The frontend
# crate's deps aren't compiled (we only build the backend bin), but Cargo
# still needs the manifests.
COPY Cargo.toml Cargo.lock ./
COPY src ./src
COPY build.rs ./build.rs
COPY backend ./backend

RUN cargo build --release --bin dnd-pc-proxy -p dnd-pc-proxy \
    && cp target/release/dnd-pc-proxy /usr/local/bin/dnd-pc-proxy

FROM gcr.io/distroless/cc-debian12:nonroot

COPY --from=builder /usr/local/bin/dnd-pc-proxy /usr/local/bin/dnd-pc-proxy

EXPOSE 8080
ENV PORT=8080 RUST_LOG=info

ENTRYPOINT ["/usr/local/bin/dnd-pc-proxy"]
