"use client";

import {
  Delete02Icon,
  PencilEdit01Icon,
  PlusSignIcon,
  UnfoldMoreIcon,
} from "@hugeicons/core-free-icons";
import { HugeiconsIcon } from "@hugeicons/react";
import * as React from "react";
import { Button } from "@tradstry/app-ui/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@tradstry/app-ui/components/ui/dialog";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuLabel,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "@tradstry/app-ui/components/ui/dropdown-menu";
import { Skeleton } from "@tradstry/app-ui/components/ui/skeleton";
import {
  useActiveWorkspace,
  useWorkspaceActions,
  useWorkspaces,
  useWorkspacesError,
  useWorkspacesLoading,
} from "./hooks";
import { ACCOUNT_ICONS } from "./icon-map";
import type { Workspace } from "./types";
import { WorkspaceDialog } from "./workspace-dialog";

const ASSET_LABELS: Record<Workspace["assetClass"], string> = {
  futures: "Futures",
  options: "Options",
  stocks: "Stocks",
  forex: "Forex",
  crypto: "Crypto",
  mixed: "Mixed",
  other: "Other",
};

export function WorkspaceSwitcher() {
  const workspaces = useWorkspaces();
  const activeWorkspace = useActiveWorkspace();
  const actions = useWorkspaceActions();
  const isLoading = useWorkspacesLoading();
  const error = useWorkspacesError();
  const [dialogOpen, setDialogOpen] = React.useState(false);
  const [editingWorkspace, setEditingWorkspace] =
    React.useState<Workspace | null>(null);
  const [deleteTarget, setDeleteTarget] = React.useState<Workspace | null>(
    null,
  );

  if (isLoading) return <Skeleton className="h-7 w-32 rounded-md" />;

  const activeIcon = activeWorkspace
    ? ACCOUNT_ICONS[activeWorkspace.icon]
    : null;

  return (
    <>
      <DropdownMenu>
        <DropdownMenuTrigger asChild>
          <Button
            variant="ghost"
            size="sm"
            className="min-w-0 max-w-56 shrink gap-1.5 px-1.5 text-muted-foreground hover:text-foreground"
          >
            {activeIcon ? (
              <span className="flex size-5 shrink-0 items-center justify-center rounded bg-muted text-foreground">
                <HugeiconsIcon
                  icon={activeIcon}
                  strokeWidth={2}
                  className="size-3.5"
                />
              </span>
            ) : null}
            <span className="hidden truncate font-medium text-foreground sm:inline">
              {error
                ? "Workspaces unavailable"
                : (activeWorkspace?.name ?? "Workspace")}
            </span>
            <HugeiconsIcon
              icon={UnfoldMoreIcon}
              strokeWidth={2}
              className="size-3"
            />
          </Button>
        </DropdownMenuTrigger>
        <DropdownMenuContent align="start" className="w-72">
          <DropdownMenuLabel>Trading workspaces</DropdownMenuLabel>
          {workspaces.map((workspace) => {
            const icon = ACCOUNT_ICONS[workspace.icon];
            return (
              <DropdownMenuItem
                key={workspace.id}
                onClick={() => actions.setActive(workspace.id)}
                className="group gap-2"
              >
                <div className="flex size-7 items-center justify-center rounded-md border">
                  {icon ? (
                    <HugeiconsIcon
                      icon={icon}
                      strokeWidth={2}
                      className="size-4"
                    />
                  ) : null}
                </div>
                <div className="min-w-0 flex-1">
                  <div className="truncate font-medium">{workspace.name}</div>
                  <div className="truncate text-xs text-muted-foreground">
                    {ASSET_LABELS[workspace.assetClass]} ·{" "}
                    {workspace.broker ?? "No brokerage connected"}
                  </div>
                </div>
                <button
                  type="button"
                  aria-label={`Edit ${workspace.name}`}
                  onClick={(event) => {
                    event.stopPropagation();
                    setEditingWorkspace(workspace);
                    setDialogOpen(true);
                  }}
                  className="rounded p-1 opacity-0 hover:bg-accent group-hover:opacity-100"
                >
                  <HugeiconsIcon
                    icon={PencilEdit01Icon}
                    strokeWidth={2}
                    className="size-4"
                  />
                </button>
                <button
                  type="button"
                  aria-label={`Delete ${workspace.name}`}
                  disabled={workspaces.length <= 1}
                  onClick={(event) => {
                    event.stopPropagation();
                    setDeleteTarget(workspace);
                  }}
                  className="rounded p-1 opacity-0 hover:bg-destructive/10 hover:text-destructive disabled:pointer-events-none group-hover:opacity-100"
                >
                  <HugeiconsIcon
                    icon={Delete02Icon}
                    strokeWidth={2}
                    className="size-4"
                  />
                </button>
              </DropdownMenuItem>
            );
          })}
          <DropdownMenuSeparator />
          <DropdownMenuItem
            className="gap-2"
            onClick={() => {
              setEditingWorkspace(null);
              setDialogOpen(true);
            }}
          >
            <HugeiconsIcon
              icon={PlusSignIcon}
              strokeWidth={2}
              className="size-4"
            />
            Create workspace
          </DropdownMenuItem>
        </DropdownMenuContent>
      </DropdownMenu>

      <WorkspaceDialog
        open={dialogOpen}
        onOpenChange={setDialogOpen}
        workspace={editingWorkspace}
        canDelete={workspaces.length > 1}
        onDelete={setDeleteTarget}
      />

      <Dialog
        open={!!deleteTarget}
        onOpenChange={(open) => !open && setDeleteTarget(null)}
      >
        <DialogContent className="sm:max-w-md">
          <DialogHeader>
            <DialogTitle>Delete workspace?</DialogTitle>
            <DialogDescription>
              This permanently removes {deleteTarget?.name} and its journal,
              notebook, playbooks, calculator data, tags, and brokerage
              connection.
            </DialogDescription>
          </DialogHeader>
          <DialogFooter>
            <Button variant="outline" onClick={() => setDeleteTarget(null)}>
              Cancel
            </Button>
            <Button
              variant="destructive"
              onClick={() => {
                if (deleteTarget) actions.delete(deleteTarget.id, workspaces);
                setDeleteTarget(null);
              }}
            >
              Delete workspace
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </>
  );
}
