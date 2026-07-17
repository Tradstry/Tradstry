"use client";

import { PlusSignIcon } from "@hugeicons/core-free-icons";
import { HugeiconsIcon } from "@hugeicons/react";
import * as React from "react";
import { toast } from "sonner";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
  DialogTrigger,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { ScrollArea } from "@/components/ui/scroll-area";
import { useCreatePlaybook } from "@/hooks/playbook";
import type { CreatePlaybookInput } from "@/lib/types/playbook";
import { RulesEditor } from "./rules-editor";

type CreatePlaybookDialogProps = {
  open?: boolean;
  onOpenChange?: (open: boolean) => void;
  trigger?: React.ReactNode;
  triggerLabel?: string;
  onCreated?: () => void;
  disabled?: boolean;
};

type PlaybookFormState = {
  name: string;
  edgeName: string;
  entryRules: string;
  exitRules: string;
  positionSizingRules: string;
  additionalRules: string;
};

const initialFormState: PlaybookFormState = {
  name: "",
  edgeName: "",
  entryRules: "",
  exitRules: "",
  positionSizingRules: "",
  additionalRules: "",
};

function Field({
  label,
  htmlFor,
  children,
}: {
  label: string;
  htmlFor?: string;
  children: React.ReactNode;
}) {
  return (
    <div className="grid gap-2">
      <Label htmlFor={htmlFor}>{label}</Label>
      {children}
    </div>
  );
}

export function CreatePlaybookDialog({
  open,
  onOpenChange,
  trigger,
  triggerLabel = "Create Playbook",
  onCreated,
  disabled = false,
}: CreatePlaybookDialogProps) {
  const createPlaybook = useCreatePlaybook();
  const [isOpen, setIsOpen] = React.useState(false);
  const [form, setForm] = React.useState<PlaybookFormState>(initialFormState);
  const [error, setError] = React.useState("");

  const controlled = open !== undefined;
  const dialogOpen = controlled ? open : isOpen;

  function setDialogOpen(next: boolean) {
    if (!controlled) {
      setIsOpen(next);
    }
    onOpenChange?.(next);

    if (!next) {
      setForm(initialFormState);
      setError("");
    }
  }

  function setField<K extends keyof PlaybookFormState>(
    key: K,
    value: PlaybookFormState[K],
  ) {
    setForm((current) => ({ ...current, [key]: value }));
    if (error) {
      setError("");
    }
  }

  function validateForm() {
    if (!form.name.trim()) return "Name is required";
    if (!form.edgeName.trim()) return "Edge name is required";
    if (!form.entryRules.trim()) return "Entry rules are required";
    if (!form.exitRules.trim()) return "Exit rules are required";
    if (!form.positionSizingRules.trim()) {
      return "Position sizing rules are required";
    }
    return "";
  }

  async function handleSubmit(event: React.FormEvent<HTMLFormElement>) {
    event.preventDefault();

    const validationError = validateForm();
    if (validationError) {
      setError(validationError);
      return;
    }

    const input: CreatePlaybookInput = {
      name: form.name.trim(),
      edgeName: form.edgeName.trim(),
      entryRules: form.entryRules.trim(),
      exitRules: form.exitRules.trim(),
      positionSizingRules: form.positionSizingRules.trim(),
      additionalRules: form.additionalRules.trim() || null,
    };
    const toastId = toast.loading("Creating playbook...");

    try {
      await createPlaybook.mutateAsync(input);
      toast.success("Playbook created.", { id: toastId });
      onCreated?.();
      setDialogOpen(false);
    } catch (submissionError) {
      toast.error(
        submissionError instanceof Error
          ? submissionError.message
          : "Failed to create playbook.",
        { id: toastId },
      );
      setError(
        submissionError instanceof Error
          ? submissionError.message
          : "Failed to create playbook",
      );
    }
  }

  return (
    <Dialog open={dialogOpen} onOpenChange={setDialogOpen}>
      {!controlled ? (
        <DialogTrigger asChild>
          {trigger ?? (
            <Button
              size="sm"
              variant="default"
              disabled={disabled}
              className="gap-2 font-semibold"
            >
              <HugeiconsIcon icon={PlusSignIcon} className="size-4" />
              {triggerLabel}
            </Button>
          )}
        </DialogTrigger>
      ) : trigger ? (
        <DialogTrigger asChild>{trigger}</DialogTrigger>
      ) : null}

      <DialogContent className="flex max-h-[calc(100svh-2rem)] flex-col overflow-hidden sm:max-w-3xl">
        <form
          onSubmit={handleSubmit}
          className="grid min-h-0 flex-1 grid-rows-[auto_1fr_auto] gap-4 overflow-hidden"
        >
          <DialogHeader className="shrink-0">
            <DialogTitle>Create playbook</DialogTitle>
            <DialogDescription>
              Define your setup, rules, and sizing criteria in one place.
            </DialogDescription>
          </DialogHeader>

          <ScrollArea className="-mx-4 min-h-0 px-4 [&>[data-radix-scroll-area-viewport]]:max-h-[60svh]">
            <div className="grid gap-4 py-4">
              <div className="grid gap-4 md:grid-cols-2">
                <Field label="Playbook name" htmlFor="playbook-name">
                  <Input
                    id="playbook-name"
                    value={form.name}
                    onChange={(event) => setField("name", event.target.value)}
                    placeholder="Breakout Long Setup"
                  />
                </Field>
                <Field label="Edge name" htmlFor="playbook-edge">
                  <Input
                    id="playbook-edge"
                    value={form.edgeName}
                    onChange={(event) =>
                      setField("edgeName", event.target.value)
                    }
                    placeholder="Gap & Follow-through"
                  />
                </Field>
              </div>

              <div className="grid gap-4 md:grid-cols-2">
                <RulesEditor
                  label="Entry rules"
                  value={form.entryRules}
                  onChange={(next) => setField("entryRules", next)}
                  placeholder="Add an entry rule…"
                  notesPlaceholder="Context that isn't a numbered rule…"
                />
                <RulesEditor
                  label="Exit rules"
                  value={form.exitRules}
                  onChange={(next) => setField("exitRules", next)}
                  placeholder="Add an exit rule…"
                  notesPlaceholder="Context that isn't a numbered rule…"
                />
              </div>

              <RulesEditor
                label="Position sizing rules"
                value={form.positionSizingRules}
                onChange={(next) => setField("positionSizingRules", next)}
                placeholder="Add a sizing rule…"
                notesPlaceholder="Context that isn't a numbered rule…"
              />

              <RulesEditor
                label="Additional rules (optional)"
                value={form.additionalRules}
                onChange={(next) => setField("additionalRules", next)}
                placeholder="Add another rule…"
                notesPlaceholder="Optional risk management or setup context…"
              />

              {error ? (
                <p className="text-sm text-destructive">{error}</p>
              ) : null}
            </div>
          </ScrollArea>

          <DialogFooter className="shrink-0">
            <Button
              type="button"
              variant="outline"
              onClick={() => setDialogOpen(false)}
              disabled={createPlaybook.isPending}
            >
              Cancel
            </Button>
            <Button
              type="submit"
              disabled={createPlaybook.isPending || disabled}
            >
              {createPlaybook.isPending ? "Saving..." : "Save playbook"}
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}
