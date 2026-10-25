# Source recipe only; preparation and downloads need separate approval.
# Context is exactly pipeline/image, never a host workspace or repository.
FROM ubuntu:24.04
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates curl tar gzip libstdc++6 python3 \
    && rm -rf /var/lib/apt/lists/*
RUN curl --fail --location --proto '=https' --tlsv1.2 \
      https://github.com/anomalyco/opencode/releases/download/v1.19.3/opencode-linux-x64-baseline.tar.gz \
      --output /tmp/opencode.tar.gz \
    && echo '531cff2441129c30f0edd5878af494108ad07f5feb46d7ee29a2a5b830a5765d  /tmp/opencode.tar.gz' | sha256sum --check --strict \
    && tar --no-same-owner -xzf /tmp/opencode.tar.gz -C /usr/local/bin opencode \
    && chmod 0555 /usr/local/bin/opencode \
    && rm /tmp/opencode.tar.gz \
    && mkdir -p /workspace /private/home /private/config /private/cache /private/data /private/state /private/tmp \
    && chown -R 1000:1000 /workspace /private
USER 1000:1000
WORKDIR /workspace
