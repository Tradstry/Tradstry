"use client";

import { useEffect, useRef } from "react";
import { useRouter, useSearchParams } from "next/navigation";
import { useGraphQL } from "@/lib/client";
import { GraphQLProvider } from "@/lib/client";
import * as brokerageService from "@/lib/service/brokerage";
import { toast } from "sonner";

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
    const accountId = searchParams.get("accountId");

    async function completeConnection() {
      if (status === "SUCCESS" && connectionId && accountId) {
        try {
          await brokerageService.completeConnection(fetcher, accountId, connectionId);
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
      <CallbackHandler />
    </GraphQLProvider>
  );
}
