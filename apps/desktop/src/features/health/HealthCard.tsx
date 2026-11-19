import type { HealthDto } from "@mr-krabs/api-client";

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
  return (
    <Card>
      <CardHeader>
        <CardTitle className="flex items-center gap-2">
          Estado del núcleo
          {loading && <Badge variant="secondary">Conectando…</Badge>}
          {health && <Badge variant="success">En línea</Badge>}
          {error && <Badge variant="destructive">Sin conexión</Badge>}
        </CardTitle>
        <CardDescription>Transporte: {transport === "tauri" ? "Tauri IPC" : "HTTP (devserver)"}</CardDescription>
      </CardHeader>
      <CardContent>
        {error && (
          <p role="alert" className="text-destructive text-sm">
            {error}
          </p>
        )}
        {health && (
          <dl className="grid grid-cols-[auto_1fr] gap-x-4 gap-y-1 text-sm">
            <dt className="text-muted-foreground">Versión</dt>
            <dd>{health.version}</dd>
            <dt className="text-muted-foreground">Esquema</dt>
            <dd>
              v{health.schemaVersion} / v{health.latestSchemaVersion}
            </dd>
            <dt className="text-muted-foreground">Base de datos</dt>
            <dd className="truncate font-mono text-xs">{health.dbPath ?? "en memoria"}</dd>
          </dl>
        )}
      </CardContent>
    </Card>
  );
}
