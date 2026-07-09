"use client";

import * as React from "react";
import { toast } from "sonner";
import { Button } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { useNotebookNotes } from "@/hooks/notebook";
import { useUpdatePrinciple } from "@/hooks/principle";
import type { PlaybookWithStats } from "@/lib/types/playbook";
import type {
  PrincipleWithStats,
  UpdatePrincipleInput,
} from "@/lib/types/principle";

const GLOBAL_VALUE = "__global__";
const NO_NOTE_VALUE = "__none__";
const TITLE_MAX = 80;

const textareaClass =
  "min-h-24 w-full rounded-md border border-input bg-input/20 px-3 py-2 text-sm outline-none transition-colors placeholder:text-muted-foreground focus-visible:border-ring focus-visible:ring-2 focus-visible:ring-ring/30";

export function EditPrincipleDialog({
  principle,
  playbooks,
  open,
  onOpenChange,
}: {
  principle: PrincipleWithStats;
  playbooks: PlaybookWithStats[];
  open: boolean;
  onOpenChange: (open: boolean) => void;
}) {
  const updatePrinciple = useUpdatePrinciple(principle.accountId);
  // Only this principle's own account's notes may be linked; the backend
  // rejects an evidence note from any other account.
  const notesQuery = useNotebookNotes(principle.accountId);
  const notes = notesQuery.data ?? [];
  const [title, setTitle] = React.useState(principle.title);
  const [theRule, setTheRule] = React.useState(principle.theRule);
  const [why, setWhy] = React.useState(principle.why);
  const [intervention, setIntervention] = React.useState(
    principle.intervention ?? "",
  );
  const [playbookId, setPlaybookId] = React.useState(
    principle.playbookId ?? GLOBAL_VALUE,
  );
  const [evidenceNoteId, setEvidenceNoteId] = React.useState(
    principle.evidenceNoteId ?? NO_NOTE_VALUE,
  );
  const [isActive, setIsActive] = React.useState(principle.isActive);
  const [error, setError] = React.useState("");

  async function handleSubmit(event: React.FormEvent<HTMLFormElement>) {
    event.preventDefault();

    if (!title.trim()) return setError("Title is required");
    if (title.trim().length > TITLE_MAX) {
      return setError("principle title must be 80 characters or less");
    }
    if (!theRule.trim()) return setError("The rule is required");
    if (!why.trim()) return setError("Why is required");

    const nextIntervention = intervention.trim();
    const nextPlaybookId = playbookId === GLOBAL_VALUE ? null : playbookId;
    const nextEvidenceNoteId =
      evidenceNoteId === NO_NOTE_VALUE ? null : evidenceNoteId;

    // The backend treats an absent optional as "leave unchanged", so clearing a
    // previously-set field requires the explicit clear* flag.
    const input: UpdatePrincipleInput = {
      title: title.trim(),
      theRule: theRule.trim(),
      why: why.trim(),
      isActive,
      intervention: nextIntervention || undefined,
      clearIntervention: !nextIntervention && principle.intervention !== null,
      playbookId: nextPlaybookId ?? undefined,
      clearPlaybook: nextPlaybookId === null && principle.playbookId !== null,
      evidenceNoteId: nextEvidenceNoteId ?? undefined,
      clearEvidenceNote:
        nextEvidenceNoteId === null && principle.evidenceNoteId !== null,
    };

    const toastId = toast.loading("Saving principle...");
    try {
      await updatePrinciple.mutateAsync({ id: principle.id, input });
      toast.success("Principle saved.", { id: toastId });
      onOpenChange(false);
    } catch (submissionError) {
      const message =
        submissionError instanceof Error
          ? submissionError.message
          : "Failed to save principle.";
      toast.error(message, { id: toastId });
      setError(message);
    }
  }

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-2xl">
        <form onSubmit={handleSubmit}>
          <DialogHeader>
            <DialogTitle>Edit principle</DialogTitle>
            <DialogDescription>
              Deactivating keeps its history but hides it from new trades.
            </DialogDescription>
          </DialogHeader>

          <div className="grid gap-4 py-4">
            <div className="grid gap-2">
              <Label htmlFor="edit-principle-title">Title</Label>
              <Input
                id="edit-principle-title"
                value={title}
                maxLength={TITLE_MAX}
                onChange={(event) => setTitle(event.target.value)}
              />
            </div>

            <div className="grid gap-2">
              <Label htmlFor="edit-principle-playbook">Applies to</Label>
              <Select value={playbookId} onValueChange={setPlaybookId}>
                <SelectTrigger id="edit-principle-playbook">
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem value={GLOBAL_VALUE}>
                    Applies to every trade
                  </SelectItem>
                  {playbooks.map((playbook) => (
                    <SelectItem key={playbook.id} value={playbook.id}>
                      {playbook.name}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
            </div>

            <div className="grid gap-2">
              <Label htmlFor="edit-principle-rule">The rule</Label>
              <textarea
                id="edit-principle-rule"
                value={theRule}
                onChange={(event) => setTheRule(event.target.value)}
                rows={3}
                className={textareaClass}
              />
            </div>

            <div className="grid gap-2">
              <Label htmlFor="edit-principle-why">Why</Label>
              <textarea
                id="edit-principle-why"
                value={why}
                onChange={(event) => setWhy(event.target.value)}
                rows={4}
                className={textareaClass}
              />
            </div>

            <div className="grid gap-2">
              <Label htmlFor="edit-principle-intervention">
                Intervention (optional)
              </Label>
              <textarea
                id="edit-principle-intervention"
                value={intervention}
                onChange={(event) => setIntervention(event.target.value)}
                rows={3}
                className={textareaClass}
              />
            </div>

            <div className="grid gap-2">
              <Label htmlFor="edit-principle-evidence">
                Evidence note (optional)
              </Label>
              {notes.length === 0 ? (
                <p className="text-xs text-muted-foreground">
                  {notesQuery.isLoading
                    ? "Loading…"
                    : "No notebook notes in this account yet."}
                </p>
              ) : (
                <Select
                  value={evidenceNoteId}
                  onValueChange={setEvidenceNoteId}
                >
                  <SelectTrigger id="edit-principle-evidence">
                    <SelectValue />
                  </SelectTrigger>
                  <SelectContent>
                    <SelectItem value={NO_NOTE_VALUE}>No note</SelectItem>
                    {notes.map((note) => (
                      <SelectItem key={note.id} value={note.id}>
                        {note.title}
                      </SelectItem>
                    ))}
                  </SelectContent>
                </Select>
              )}
            </div>

            <div className="flex items-center gap-2">
              <Checkbox
                id="edit-principle-active"
                checked={isActive}
                onCheckedChange={(checked) => setIsActive(checked === true)}
              />
              <Label htmlFor="edit-principle-active">
                Active (shown when logging trades)
              </Label>
            </div>

            {error ? <p className="text-sm text-destructive">{error}</p> : null}
          </div>

          <DialogFooter>
            <Button
              type="button"
              variant="outline"
              onClick={() => onOpenChange(false)}
              disabled={updatePrinciple.isPending}
            >
              Cancel
            </Button>
            <Button type="submit" disabled={updatePrinciple.isPending}>
              {updatePrinciple.isPending ? "Saving..." : "Save principle"}
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}
