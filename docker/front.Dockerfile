# Dev image for the React UI (docker compose `front`). Source is bind-mounted.
FROM node:22-bookworm-slim

ENV PNPM_HOME=/pnpm \
    PATH=/pnpm:$PATH \
    CI=true
RUN corepack enable && corepack prepare pnpm@10.28.0 --activate

WORKDIR /workspace
EXPOSE 5173
CMD ["sh", "-c", "pnpm install && pnpm --filter @mr-krabs/desktop exec vite --host 0.0.0.0"]
