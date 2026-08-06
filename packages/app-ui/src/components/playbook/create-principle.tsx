"use client";

import { PlusSignIcon } from "@hugeicons/core-free-icons";
import { HugeiconsIcon } from "@hugeicons/react";
import * as React from "react";
import { toast } from "sonner";
import { Button } from "@tradstry/app-ui/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
  DialogTrigger,
} from "@tradstry/app-ui/components/ui/dialog";
import { Input } from "@tradstry/app-ui/components/ui/input";
import { Label } from "@tradstry/app-ui/components/ui/label";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@tradstry/app-ui/components/ui/select";
import { useNotebookNotes } from "@tradstry/app-ui/hooks/notebook";
import { useCreatePrinciple } from "@tradstry/app-ui/hooks/principle";
import type { PlaybookWithStats } from "@tradstry/app-ui/lib/types/playbook";
import type { CreatePrincipleInput } from "@tradstry/app-ui/lib/types/principle";

const GLOBAL_VALUE = "__global__";
const NO_NOTE_VALUE = "__none__";
const TITLE_MAX = 80;

const textareaClass =
  "min-h-24 w-full rounded-md border border-input bg-input/20 px-3 py-2 text-sm outline-none transition-colors placeholder:text-muted-foreground focus-visible:border-ring focus-visible:ring-2 focus-visible:ring-ring/30";

type FormState = {
  title: string;
  theRule: string;
  why: string;
  intervention: string;
  playbookId: string;
  evidenceNoteId: string;
};

const initialFormState: FormState = {
  title: "",
  theRule: "",
  why: "",
  intervention: "",
  playbookId: GLOBAL_VALUE,
  evidenceNoteId: NO_NOTE_VALUE,
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

export function CreatePrincipleDialog({
  workspaceId,
  playbooks,
}: {
  workspaceId: string;
  playbooks: PlaybookWithStats[];
}) {
  const createPrinciple = useCreatePrinciple(workspaceId);
  // Only this account's notes may be linked: the backend rejects an evidence
  // note belonging to a different account.
  const notesQuery = useNotebookNotes(workspaceId);
  const notes = notesQuery.data ?? [];
  const [isOpen, setIsOpen] = React.useState(false);
  const [form, setForm] = React.useState<FormState>(initialFormState);
  const [error, setError] = React.useState("");

  function setDialogOpen(next: boolean) {
    setIsOpen(next);
    if (!next) {
      setForm(initialFormState);
      setError("");
    }
  }

  function setField<K extends keyof FormState>(key: K, value: FormState[K]) {
    setForm((current) => ({ ...current, [key]: value }));
    if (error) setError("");
  }

  function validateForm() {
    if (!form.title.trim()) return "Title is required";
    // Same limit and wording as the backend's validate_title_length.
    if (form.title.trim().length > TITLE_MAX) {
      return "principle title must be 80 characters or less";
    }
    if (!form.theRule.trim()) return "The rule is required";
    if (!form.why.trim()) return "Why is required";
    return "";
  }

  async function handleSubmit(event: React.FormEvent<HTMLFormElement>) {
    event.preventDefault();

    const validationError = validateForm();
    if (validationError) {
      setError(validationError);
      return;
    }

    const input: CreatePrincipleInput = {
      workspaceId,
      title: form.title.trim(),
      theRule: form.theRule.trim(),
      why: form.why.trim(),
      intervention: form.intervention.trim() || null,
      playbookId: form.playbookId === GLOBAL_VALUE ? null : form.playbookId,
      evidenceNoteId:
        form.evidenceNoteId === NO_NOTE_VALUE ? null : form.evidenceNoteId,
    };
    const toastId = toast.loading("Creating principle...");

    try {
      await createPrinciple.mutateAsync(input);
      toast.success("Principle created.", { id: toastId });
      setDialogOpen(false);
    } catch (submissionError) {
      const message =
        submissionError instanceof Error
          ? submissionError.message
          : "Failed to create principle.";
      toast.error(message, { id: toastId });
      setError(message);
    }
  }

  return (
    <Dialog open={isOpen} onOpenChange={setDialogOpen}>
      <DialogTrigger asChild>
        <Button size="sm" variant="default" className="gap-2 font-semibold">
          <HugeiconsIcon icon={PlusSignIcon} className="size-4" />
          New principle
        </Button>
      </DialogTrigger>

      <DialogContent className="sm:max-w-2xl">
        <form onSubmit={handleSubmit}>
          <DialogHeader>
            <DialogTitle>New principle</DialogTitle>
            <DialogDescription>
              A rule you will read before trading, and tick when you break it.
            </DialogDescription>
          </DialogHeader>

          <div className="grid gap-4 py-4">
            <Field label="Title" htmlFor="principle-title">
              <Input
                id="principle-title"
                value={form.title}
                maxLength={TITLE_MAX}
                onChange={(event) => setField("title", event.target.value)}
                placeholder="30-min rule"
              />
            </Field>

            <Field label="Applies to" htmlFor="principle-playbook">
              <Select
                value={form.playbookId}
                onValueChange={(value) => setField("playbookId", value)}
              >
                <SelectTrigger id="principle-playbook">
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
            </Field>

            <Field label="The rule" htmlFor="principle-rule">
              <textarea
                id="principle-rule"
                value={form.theRule}
                onChange={(event) => setField("theRule", event.target.value)}
                placeholder="Do not touch a position between 9:30 and 10:00 ET."
                rows={3}
                className={textareaClass}
              />
            </Field>

            <Field label="Why" htmlFor="principle-why">
              <textarea
                id="principle-why"
                value={form.why}
                onChange={(event) => setField("why", event.target.value)}
                placeholder="The evidence. What breaking it has cost you."
                rows={4}
                className={textareaClass}
              />
            </Field>

            <Field
              label="Intervention (optional)"
              htmlFor="principle-intervention"
            >
              <textarea
                id="principle-intervention"
                value={form.intervention}
                onChange={(event) =>
                  setField("intervention", event.target.value)
                }
                placeholder="The physical change that enforces the rule."
                rows={3}
                className={textareaClass}
              />
            </Field>

            <Field
              label="Evidence note (optional)"
              htmlFor="principle-evidence"
            >
              {notes.length === 0 ? (
                <p className="text-xs text-muted-foreground">
                  {notesQuery.isLoading
                    ? "Loading…"
                    : "No notebook notes in this workspace yet."}
                </p>
              ) : (
                <Select
                  value={form.evidenceNoteId}
                  onValueChange={(value) => setField("evidenceNoteId", value)}
                >
                  <SelectTrigger id="principle-evidence">
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
            </Field>

            {error ? <p className="text-sm text-destructive">{error}</p> : null}
          </div>

          <DialogFooter>
            <Button
              type="button"
              variant="outline"
              onClick={() => setDialogOpen(false)}
              disabled={createPrinciple.isPending}
            >
              Cancel
            </Button>
            <Button type="submit" disabled={createPrinciple.isPending}>
              {createPrinciple.isPending ? "Saving..." : "Save principle"}
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}
