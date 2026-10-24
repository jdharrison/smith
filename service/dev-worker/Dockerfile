# Operator-only recipe; context is service/, not this directory. Never built by the runtime.
FROM ubuntu:24.04
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates curl tar gzip libstdc++6 \
    && rm -rf /var/lib/apt/lists/*
RUN curl --fail --location --proto '=https' --tlsv1.2 \
      https://github.com/anomalyco/opencode/releases/download/v1.19.3/opencode-linux-x64-baseline.tar.gz \
      --output /tmp/opencode.tar.gz \
    && echo '531cff2441129c30f0edd5878af494108ad07f5feb46d7ee29a2a5b830a5765d  /tmp/opencode.tar.gz' | sha256sum --check --strict \
    && tar --no-same-owner -xzf /tmp/opencode.tar.gz -C /usr/local/bin opencode \
    && chmod 0755 /usr/local/bin/opencode \
    && rm /tmp/opencode.tar.gz
COPY --chown=0:0 dist/smith-service /usr/local/bin/smith-service
RUN chmod 0755 /usr/local/bin/smith-service \
    && mkdir -p /workspace /etc/smith-opencode/opencode \
    && printf 'smith-receipt-worker-v1\n' > /etc/smith-dev-worker \
    && printf 'node_modules\npackage.json\npackage-lock.json\n' > /etc/smith-opencode/opencode/.gitignore \
    && chmod -R a-w /etc/smith-opencode /etc/smith-dev-worker \
    && chown 1000:1000 /workspace
USER 1000:1000
WORKDIR /workspace
ENV SMITH_DEV_WORKER=smith-receipt-worker-v1
ENTRYPOINT ["/usr/local/bin/smith-service", "dev-worker"]
