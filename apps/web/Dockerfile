FROM node:24-alpine AS builder
WORKDIR /app

COPY package.json package-lock.json tsconfig.base.json ./
COPY apps/web/package.json apps/web/package.json

RUN npm ci

COPY apps/web apps/web

RUN npm run build:web

FROM nginx:1.27-alpine AS runner
WORKDIR /usr/share/nginx/html

COPY apps/web/nginx.conf /etc/nginx/conf.d/default.conf
COPY --from=builder /app/apps/web/dist ./

EXPOSE 3000
CMD ["nginx", "-g", "daemon off;"]
