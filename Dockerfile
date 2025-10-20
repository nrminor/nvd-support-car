# Build stage using Nix
FROM nixos/nix:latest AS builder

RUN echo "experimental-features = nix-command flakes" >> /etc/nix/nix.conf

WORKDIR /build
COPY . .

RUN nix build .#default --no-link --print-out-paths > /tmp/build-path && \
    cp "$(cat /tmp/build-path)/bin/nvd-support-car" /tmp/nvd-support-car

# Final stage - minimal runtime image with libc for PostgreSQL client
FROM gcr.io/distroless/cc-debian12:nonroot

COPY --from=builder /tmp/nvd-support-car /usr/local/bin/nvd-support-car

ENTRYPOINT ["/usr/local/bin/nvd-support-car"]
