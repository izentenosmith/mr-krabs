# Dev image for the React UI (docker compose `front`). Source is bind-mounted.
FROM node:22-bookworm-slim

ENV PNPM_HOME=/pnpm \
    PATH=/pnpm:$PATH \
    CI=true
RUN corepack enable && corepack prepare pnpm@10.28.0 --activate

WORKDIR /workspace
EXPOSE 5173
# No HEALTHCHECK: dev-only image, nothing gates on the UI being ready.
# `exec` hands PID 1's role to the server so `docker stop`'s SIGTERM reaches it.
CMD ["sh", "-c", "pnpm install && exec pnpm --filter @mr-krabs/desktop exec vite --host 0.0.0.0"]
