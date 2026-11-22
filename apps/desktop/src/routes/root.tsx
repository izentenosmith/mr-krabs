import { Outlet } from "@tanstack/react-router";
import { useQueryClient } from "@tanstack/react-query";
import { useTranslation } from "react-i18next";
import { CircleAlertIcon } from "lucide-react";
import { ApiError, api } from "@mr-krabs/api-client";

import { Button } from "@/components/ui/button";
import { Toaster } from "@/components/ui/sonner";
import { NotAvailable } from "@/components/NotAvailable";
import { OnboardingForm } from "@/features/onboarding/OnboardingForm";
import { bootstrapKey, useBootstrap } from "@/features/shell/bootstrap";
import { StatusScreen } from "@/features/shell/StatusScreen";
import { errorCopyKey, isNotImplemented } from "@/lib/api-errors";

/**
 * Root: gates the app on `app_bootstrap`. No household → onboarding; otherwise the layout
 * route (sidebar + content) renders through the outlet.
 */
export function RootLayout() {
  return (
    <>
      <BootstrapGate />
      <Toaster />
    </>
  );
}

function BootstrapGate() {
  const { t } = useTranslation();
  const qc = useQueryClient();
  const bootstrap = useBootstrap();

  if (bootstrap.isPending) {
    return (
      <StatusScreen busy>
        <p className="text-muted-foreground text-sm">{t("shell.loading")}</p>
      </StatusScreen>
    );
  }
  if (bootstrap.isError) {
    if (isNotImplemented(bootstrap.error)) {
      return (
        <StatusScreen>
          <NotAvailable />
        </StatusScreen>
      );
    }
    return (
      <StatusScreen>
        <div role="alert" className="text-destructive flex items-start gap-2 text-sm">
          <CircleAlertIcon className="mt-0.5 size-4 shrink-0" aria-hidden />
          <div>
            <p className="font-medium">{t("shell.bootstrapError")}</p>
            <p>{t(errorCopyKey(ApiError.from(bootstrap.error).code))}</p>
          </div>
        </div>
        <Button variant="outline" size="sm" className="mt-4" onClick={() => void bootstrap.refetch()}>
          {t("shell.retry")}
        </Button>
      </StatusScreen>
    );
  }
  if (!bootstrap.data.household) {
    return (
      <OnboardingForm
        onSubmit={async (input) => {
          await api.household.create(input);
          await qc.invalidateQueries({ queryKey: bootstrapKey });
        }}
      />
    );
  }
  return <Outlet />;
}
