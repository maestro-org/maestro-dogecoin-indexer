# Dogecoin Core built from the official release binaries, checksum-verified
# against the signed SHA256SUMS of the v1.14.9 release.
FROM debian:bookworm-slim AS fetch

ARG VERSION=1.14.9
ARG TARGETARCH

RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates curl && rm -rf /var/lib/apt/lists/*

RUN set -eux; \
    case "${TARGETARCH}" in \
      amd64) PLATFORM="x86_64-linux-gnu"; \
             SHA256="4f227117b411a7c98622c970986e27bcfc3f547a72bef65e7d9e82989175d4f8" ;; \
      arm64) PLATFORM="aarch64-linux-gnu"; \
             SHA256="6928c895a20d0bcb6d5c7dcec753d35c884a471aaf8ad4242a89a96acb4f2985" ;; \
      *) echo "unsupported architecture: ${TARGETARCH}" >&2; exit 1 ;; \
    esac; \
    curl -fsSL -o /tmp/dogecoin.tar.gz \
      "https://github.com/dogecoin/dogecoin/releases/download/v${VERSION}/dogecoin-${VERSION}-${PLATFORM}.tar.gz"; \
    echo "${SHA256}  /tmp/dogecoin.tar.gz" | sha256sum -c -; \
    tar -xzf /tmp/dogecoin.tar.gz -C /tmp; \
    install -m 0755 "/tmp/dogecoin-${VERSION}/bin/dogecoind" "/tmp/dogecoin-${VERSION}/bin/dogecoin-cli" /usr/local/bin/

FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates tini && rm -rf /var/lib/apt/lists/* \
    && useradd --system --uid 1000 --create-home dogecoin \
    && mkdir -p /data && chown dogecoin:dogecoin /data

COPY --from=fetch /usr/local/bin/dogecoind /usr/local/bin/dogecoin-cli /usr/local/bin/

USER dogecoin
# mainnet: P2P 22556, RPC 22555; testnet: P2P 44556, RPC 44555
EXPOSE 22555 22556 44555 44556
ENTRYPOINT ["/usr/bin/tini", "-g", "--", "dogecoind"]
CMD ["-datadir=/data", "-printtoconsole"]
