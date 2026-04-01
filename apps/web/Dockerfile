FROM quay.io/fedora/fedora:43 AS builder
WORKDIR /app

RUN dnf install -y nodejs npm \
    && dnf clean all

COPY package.json package-lock.json tsconfig.base.json ./
COPY apps/web/package.json apps/web/package.json

RUN npm ci

COPY apps/web apps/web

ARG FINDVERSE_SITE_NAME=FindVerse
ENV VITE_FINDVERSE_SITE_NAME=${FINDVERSE_SITE_NAME}

RUN npm run build:web

FROM quay.io/nginx/nginx-unprivileged:stable-alpine AS runner
WORKDIR /usr/share/nginx/html

ARG FINDVERSE_CONTROL_API_PROXY=http://control-api:8080
ARG FINDVERSE_QUERY_API_PROXY=http://query-api:8081
ARG FINDVERSE_TASK_API_PROXY=http://task-api:8082
USER root

COPY apps/web/nginx.conf /tmp/default.conf.template
COPY --from=builder /app/apps/web/dist ./

RUN sed "s|__FINDVERSE_CONTROL_API_PROXY__|${FINDVERSE_CONTROL_API_PROXY}|g; s|__FINDVERSE_QUERY_API_PROXY__|${FINDVERSE_QUERY_API_PROXY}|g; s|__FINDVERSE_TASK_API_PROXY__|${FINDVERSE_TASK_API_PROXY}|g" /tmp/default.conf.template > /etc/nginx/conf.d/default.conf

USER 101

EXPOSE 3000
CMD ["nginx", "-g", "daemon off;"]
