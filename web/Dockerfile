FROM node:24-bookworm AS builder

ENV COREPACK_ENABLE_DOWNLOAD_PROMPT=0

RUN corepack enable pnpm

WORKDIR /usr/src/app

COPY package.json pnpm-workspace.yaml pnpm-lock.yaml .pnpmfile.cjs ./
COPY open-api/typescript-sdk ./open-api/typescript-sdk
COPY i18n ./i18n
COPY web ./web

RUN pnpm install --filter @immich/sdk --filter immich-web --frozen-lockfile
RUN pnpm --filter @immich/sdk build
RUN pnpm --filter immich-web build

FROM nginx:1.27-alpine

COPY web/nginx.conf /etc/nginx/conf.d/default.conf
COPY --from=builder /usr/src/app/web/build /usr/share/nginx/html

EXPOSE 3000

CMD ["nginx", "-g", "daemon off;"]
