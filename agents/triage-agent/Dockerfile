# Python agent on a distroless runtime — Gemini via google-genai (no Node/CLI).
# Builder uses python:3.11-slim to match distroless/python3-debian12 (Python 3.11).
# Build context is the REPO ROOT so grounding/ is bundled:
#   gcloud builds submit . --config=agents/triage-agent/cloudbuild.yaml --substitutions=_IMAGE=IMG
FROM python:3.11-slim AS build
WORKDIR /app
COPY agents/triage-agent/requirements.txt ./
RUN pip install --no-cache-dir --target=/install -r requirements.txt

FROM gcr.io/distroless/python3-debian12
WORKDIR /app
COPY --from=build /install /app/deps
COPY agents/triage-agent/app ./app
COPY grounding ./grounding
ENV PYTHONPATH=/app/deps
ENV GROUNDING_DIR=/app/grounding
ENV PORT=8080
ENTRYPOINT ["/usr/bin/python3", "-m", "uvicorn", "app.main:app", "--host", "0.0.0.0", "--port", "8080"]
