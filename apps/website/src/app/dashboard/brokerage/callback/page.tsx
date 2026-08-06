"use client";

import { useRouter, useSearchParams } from "next/navigation";
import { Suspense, useEffect, useRef } from "react";
import { toast } from "sonner";
import { GraphQLProvider, useGraphQL } from "@/lib/client";
import * as brokerageService from "@tradstry/app-ui/lib/service/brokerage";

function CallbackHandler() {
  const searchParams = useSearchParams();
  const router = useRouter();
  const fetcher = useGraphQL();
  const didRun = useRef(false);

  useEffect(() => {
    if (didRun.current) return;
    didRun.current = true;

    const status = searchParams.get("status");
    const connectionId = searchParams.get("connection_id");
    const workspaceId = searchParams.get("workspaceId");

    async function completeConnection() {
      if (status === "SUCCESS" && connectionId && workspaceId) {
        try {
          await brokerageService.completeConnection(
            fetcher,
            workspaceId,
            connectionId,
          );
          toast.success("Brokerage connected!");
        } catch {
          toast.error("Connected but failed to save. Please try again.");
        }
      } else if (status === "ERROR") {
        const errorCode = searchParams.get("error_code") ?? "Unknown error";
        toast.error(`Connection failed: ${errorCode}`);
      } else if (status === "ABANDONED") {
        toast.info("Connection cancelled.");
      }

      router.replace("/dashboard/brokerage");
    }

    completeConnection();
  }, [searchParams, router, fetcher]);

  return (
    <div className="flex flex-1 items-center justify-center p-6">
      <p className="text-sm text-muted-foreground">Completing connection...</p>
    </div>
  );
}

export default function BrokerageCallbackPage() {
  return (
    <GraphQLProvider>
      <Suspense
        fallback={
          <div className="flex flex-1 items-center justify-center p-6">
            <p className="text-sm text-muted-foreground">Loading...</p>
          </div>
        }
      >
        <CallbackHandler />
      </Suspense>
    </GraphQLProvider>
  );
}
