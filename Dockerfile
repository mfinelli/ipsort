FROM rust:slim AS source
WORKDIR /ipsort
COPY . /ipsort
RUN cargo vendor --locked

FROM source AS build
RUN cargo build --frozen --release --verbose

FROM build AS test
RUN cargo test

FROM debian:stable-slim

LABEL org.opencontainers.image.title=ipsort
LABEL org.opencontainers.image.version=v1.0.0
LABEL org.opencontainers.image.description="versitile ip address sorting tool"
LABEL org.opencontainers.image.url=https://github.com/mfinelli/ipsort
LABEL org.opencontainers.image.source=https://github.com/mfinelli/ipsort
LABEL org.opencontainers.image.licenses=GPL-3.0-or-later

RUN useradd -r -U -m ipsort
COPY --from=source /ipsort /usr/src/ipsort
COPY --from=build /ipsort/target/release/ipsort /usr/bin/ipsort
USER ipsort
CMD ["ipsort"]
