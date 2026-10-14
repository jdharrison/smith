FROM python:3.12-slim-bookworm
ENV PYTHONDONTWRITEBYTECODE=1 PYTHONUNBUFFERED=1 \
    HF_HUB_OFFLINE=1 TRANSFORMERS_OFFLINE=1 HF_HUB_DISABLE_TELEMETRY=1 \
    DO_NOT_TRACK=1 OMP_NUM_THREADS=2 HOME=/tmp
RUN apt-get update && apt-get install -y --no-install-recommends \
      libgomp1 libstdc++6 libespeak-ng1 espeak-ng-data libsndfile1 \
    && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY requirements.txt ./requirements.txt
RUN python -m pip install --no-cache-dir -r requirements.txt
COPY api.py engines.py serve.py healthcheck.py ./
USER 10001:10001
CMD ["python", "serve.py", "stt"]
