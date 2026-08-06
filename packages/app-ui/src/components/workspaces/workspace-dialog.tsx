"use client";

import {
  Cancel01Icon,
  Delete02Icon,
  PencilEdit01Icon,
  PlusSignIcon,
} from "@hugeicons/core-free-icons";
import { HugeiconsIcon } from "@hugeicons/react";
import * as React from "react";
import { Button } from "@tradstry/app-ui/components/ui/button";
import {
  Dialog,
  DialogClose,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@tradstry/app-ui/components/ui/dialog";
import { Input } from "@tradstry/app-ui/components/ui/input";
import { Label } from "@tradstry/app-ui/components/ui/label";
import { RadioGroup, RadioGroupItem } from "@tradstry/app-ui/components/ui/radio-group";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@tradstry/app-ui/components/ui/select";
import { useWorkspaceActions, useWorkspaces } from "./hooks";
import { ACCOUNT_ICONS, DEFAULT_ICON, ICON_OPTIONS } from "./icon-map";
import type { AssetClass, Currency, RiskProfile, Workspace } from "./types";
import { CURRENCIES } from "./types";

const ASSET_CLASS_OPTIONS: Array<{ value: AssetClass; label: string }> = [
  { value: "futures", label: "Futures" },
  { value: "options", label: "Options" },
  { value: "stocks", label: "Stocks" },
  { value: "forex", label: "Forex" },
  { value: "crypto", label: "Crypto" },
  { value: "mixed", label: "Mixed" },
  { value: "other", label: "Other" },
];

const RISK_OPTIONS: Array<{ value: RiskProfile; label: string }> = [
  { value: "conservative", label: "Conservative" },
  { value: "moderate", label: "Moderate" },
  { value: "aggressive", label: "Aggressive" },
];

interface WorkspaceDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  workspace?: Workspace | null;
  canDelete?: boolean;
  onDelete?: (workspace: Workspace) => void;
}

export function WorkspaceDialog({
  open,
  onOpenChange,
  workspace,
  canDelete = false,
  onDelete,
}: WorkspaceDialogProps) {
  const isEditing = !!workspace;
  const workspaces = useWorkspaces();
  const actions = useWorkspaceActions();

  const [name, setName] = React.useState("");
  const [currency, setCurrency] = React.useState<Currency>("USD");
  const [assetClass, setAssetClass] = React.useState<AssetClass>("mixed");
  const [riskProfile, setRiskProfile] = React.useState<RiskProfile>("moderate");
  const [icon, setIcon] = React.useState(DEFAULT_ICON);
  const [error, setError] = React.useState("");

  // biome-ignore lint/correctness/useExhaustiveDependencies: reset form state when dialog opens
  React.useEffect(() => {
    if (workspace) {
      setName(workspace.name);
      setCurrency(workspace.currency);
      setAssetClass(workspace.assetClass);
      setRiskProfile(workspace.riskProfile);
      setIcon(workspace.icon);
    } else {
      setName("");
      setCurrency("USD");
      setAssetClass("mixed");
      setRiskProfile("moderate");
      setIcon(DEFAULT_ICON);
    }
    setError("");
  }, [workspace, open]);

  function handleSubmit(e: React.FormEvent) {
    e.preventDefault();

    const trimmedName = name.trim();
    if (!trimmedName) {
      setError("Workspace name is required");
      return;
    }
    if (trimmedName.length > 50) {
      setError("Workspace name must be 50 characters or less");
      return;
    }

    const isDuplicate = workspaces.some(
      (a) =>
        a.name.toLowerCase() === trimmedName.toLowerCase() &&
        a.id !== workspace?.id,
    );
    if (isDuplicate) {
      setError("A workspace with this name already exists");
      return;
    }

    if (isEditing && workspace) {
      actions.update(workspace.id, {
        name: trimmedName,
        currency,
        assetClass,
        riskProfile,
        icon,
      });
    } else {
      actions.create({
        name: trimmedName,
        currency,
        assetClass,
        riskProfile,
        icon,
        broker: null,
      });
    }

    onOpenChange(false);
  }

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent
        className="gap-0 overflow-hidden p-0 sm:max-w-lg"
        showCloseButton={false}
      >
        <form onSubmit={handleSubmit}>
          <DialogClose asChild>
            <Button
              type="button"
              variant="ghost"
              size="icon-sm"
              className="absolute top-7 right-5 z-10 text-muted-foreground"
            >
              <HugeiconsIcon icon={Cancel01Icon} strokeWidth={2} />
              <span className="sr-only">Close</span>
            </Button>
          </DialogClose>
          <DialogHeader className="border-b px-6 py-5 pr-14 text-left">
            <div className="flex items-center gap-3">
              <span className="flex size-10 shrink-0 items-center justify-center rounded-xl border bg-muted/60 text-foreground">
                <HugeiconsIcon
                  icon={isEditing ? PencilEdit01Icon : PlusSignIcon}
                  strokeWidth={2}
                  className="size-5"
                />
              </span>
              <div className="min-w-0 space-y-1">
                <DialogTitle className="text-base">
                  {isEditing ? "Edit workspace" : "Create workspace"}
                </DialogTitle>
                <DialogDescription className="max-w-sm">
                  {isEditing
                    ? "Change how this workspace is named and organized."
                    : "Create a focused space for a specific kind of trading."}
                </DialogDescription>
              </div>
            </div>
          </DialogHeader>

          <div className="grid gap-5 px-6 py-5">
            <div className="grid gap-2">
              <Label htmlFor="workspace-name">Name</Label>
              <Input
                id="workspace-name"
                className="h-9 px-3 text-sm md:text-sm"
                value={name}
                onChange={(e) => {
                  setName(e.target.value);
                  setError("");
                }}
                placeholder="e.g., Futures workspace"
                maxLength={50}
              />
              {error && <p className="text-sm text-destructive">{error}</p>}
            </div>

            <div className="grid gap-4 sm:grid-cols-2">
              <div className="grid gap-2">
                <Label>Trading type</Label>
                <Select
                  value={assetClass}
                  onValueChange={(value) => setAssetClass(value as AssetClass)}
                >
                  <SelectTrigger className="w-full px-3 data-[size=default]:h-9">
                    <SelectValue />
                  </SelectTrigger>
                  <SelectContent>
                    {ASSET_CLASS_OPTIONS.map((option) => (
                      <SelectItem key={option.value} value={option.value}>
                        {option.label}
                      </SelectItem>
                    ))}
                  </SelectContent>
                </Select>
              </div>

              <div className="grid gap-2">
                <Label>Currency</Label>
                <Select
                  value={currency}
                  onValueChange={(value) => setCurrency(value as Currency)}
                >
                  <SelectTrigger className="w-full px-3 data-[size=default]:h-9">
                    <SelectValue />
                  </SelectTrigger>
                  <SelectContent>
                    {CURRENCIES.map((option) => (
                      <SelectItem key={option} value={option}>
                        {option}
                      </SelectItem>
                    ))}
                  </SelectContent>
                </Select>
              </div>
              <p className="text-xs text-muted-foreground sm:col-span-2">
                Journal entries, analytics, playbooks, and brokerage data stay
                inside this workspace.
              </p>
            </div>

            <div className="grid gap-2">
              <Label>Risk profile</Label>
              <RadioGroup
                value={riskProfile}
                onValueChange={(value) => setRiskProfile(value as RiskProfile)}
                className="grid grid-cols-1 gap-2 sm:grid-cols-3"
              >
                {RISK_OPTIONS.map((option) => {
                  const selected = riskProfile === option.value;
                  const id = `risk-${option.value}`;
                  return (
                    <Label
                      key={option.value}
                      htmlFor={id}
                      className={`cursor-pointer rounded-lg border px-3 py-3 font-normal transition-colors ${
                        selected
                          ? "border-primary bg-primary/5 text-foreground"
                          : "border-border bg-input/10 text-muted-foreground hover:bg-muted/60 hover:text-foreground"
                      }`}
                    >
                      <RadioGroupItem value={option.value} id={id} />
                      {option.label}
                    </Label>
                  );
                })}
              </RadioGroup>
            </div>

            <div className="grid gap-2">
              <Label>Workspace icon</Label>
              <div className="grid grid-cols-5 gap-2 sm:grid-cols-10">
                {ICON_OPTIONS.map((key) => {
                  const iconData = ACCOUNT_ICONS[key];
                  if (!iconData) return null;
                  return (
                    <button
                      key={key}
                      type="button"
                      aria-label={`Use ${key.replaceAll("-", " ")} icon`}
                      aria-pressed={icon === key}
                      onClick={() => setIcon(key)}
                      className={`flex aspect-square w-full items-center justify-center rounded-lg border transition-colors ${
                        icon === key
                          ? "border-primary bg-primary text-primary-foreground shadow-sm"
                          : "border-border bg-input/10 text-muted-foreground hover:border-primary/50 hover:bg-muted hover:text-foreground"
                      }`}
                    >
                      <HugeiconsIcon
                        icon={iconData}
                        strokeWidth={2}
                        className="size-4.5"
                      />
                    </button>
                  );
                })}
              </div>
            </div>
          </div>

          <DialogFooter className="border-t bg-muted/20 px-6 py-4 sm:items-center sm:justify-between">
            {isEditing && workspace ? (
              <Button
                type="button"
                variant="ghost"
                className="text-destructive hover:bg-destructive/10 hover:text-destructive"
                disabled={!canDelete}
                title={
                  canDelete
                    ? "Delete this workspace"
                    : "You must keep at least one workspace"
                }
                onClick={() => {
                  onOpenChange(false);
                  onDelete?.(workspace);
                }}
              >
                <HugeiconsIcon icon={Delete02Icon} strokeWidth={2} />
                Delete workspace
              </Button>
            ) : (
              <span />
            )}
            <div className="flex justify-end gap-2">
              <Button
                type="button"
                variant="outline"
                onClick={() => onOpenChange(false)}
              >
                Cancel
              </Button>
              <Button type="submit">
                {isEditing ? "Save changes" : "Create workspace"}
              </Button>
            </div>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}
