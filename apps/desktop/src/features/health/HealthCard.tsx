import type { HealthDto } from "@mr-krabs/api-client";
import { useTranslation } from "react-i18next";

import { Badge } from "@/components/ui/badge";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";

export interface HealthCardProps {
  health?: HealthDto;
  error?: string;
  loading?: boolean;
  transport: string;
}

/** Presentational: renders what the core reported. No data fetching, no logic. */
export function HealthCard({ health, error, loading, transport }: HealthCardProps) {
  const { t } = useTranslation();
  return (
    <Card>
      <CardHeader>
        <CardTitle className="flex items-center gap-2">
          {t("health.title")}
          {loading && <Badge variant="secondary">{t("health.connecting")}</Badge>}
          {health && <Badge variant="success">{t("health.online")}</Badge>}
          {error && <Badge variant="destructive">{t("health.offline")}</Badge>}
        </CardTitle>
        <CardDescription>
          {t("health.transport", {
            name: transport === "tauri" ? t("health.transportTauri") : t("health.transportHttp"),
          })}
        </CardDescription>
      </CardHeader>
      <CardContent>
        {error && (
          <p role="alert" className="text-destructive text-sm">
            {error}
          </p>
        )}
        {health && (
          <dl className="grid grid-cols-[auto_1fr] gap-x-4 gap-y-1 text-sm">
            <dt className="text-muted-foreground">{t("health.version")}</dt>
            <dd>{health.version}</dd>
            <dt className="text-muted-foreground">{t("health.schema")}</dt>
            <dd>
              v{health.schemaVersion} / v{health.latestSchemaVersion}
            </dd>
            <dt className="text-muted-foreground">{t("health.database")}</dt>
            <dd className="truncate font-mono text-xs">{health.dbPath ?? t("health.inMemory")}</dd>
          </dl>
        )}
      </CardContent>
    </Card>
  );
}
