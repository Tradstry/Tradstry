"use client";

import { useAuth } from "@clerk/nextjs";
import * as React from "react";
import { toast } from "sonner";
import { Section, Spinner } from "@/components/account/shared";
import { Button } from "@/components/ui/button";
import { getBackendBaseUrl } from "@/lib/client/backend-connection";

export function ExportSection() {
  const { getToken } = useAuth();
  const [busy, setBusy] = React.useState(false);

  async function download() {
    setBusy(true);
    try {
      const token = await getToken();
      const response = await fetch(`${getBackendBaseUrl()}/export`, {
        headers: token ? { Authorization: `Bearer ${token}` } : undefined,
      });
      if (!response.ok) throw new Error(`Export failed (${response.status})`);

      const blob = await response.blob();
      const url = URL.createObjectURL(blob);
      const link = document.createElement("a");
      link.href = url;
      link.download = `tradstry-export-${new Date().toISOString().slice(0, 10)}.json`;
      link.click();
      URL.revokeObjectURL(url);
      toast.success("Your export has downloaded.");
    } catch {
      toast.error("Could not build your export. Try again in a moment.");
    } finally {
      setBusy(false);
    }
  }

  return (
    <Section
      title="Export your data"
      description="Download everything Tradstry holds about you as a single JSON file — trades, journal, playbooks, principles, tags and notebook. Media links stay valid for seven days."
      footer={
        <Button size="sm" variant="outline" onClick={download} disabled={busy}>
          {busy ? <Spinner /> : null}
          Download my data
        </Button>
      }
    >
      <p className="text-xs text-muted-foreground">
        Nothing is deleted by exporting.
      </p>
    </Section>
  );
}
