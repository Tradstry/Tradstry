import { useState, type ReactNode } from "react";
import { BookOpenIcon } from "@phosphor-icons/react";
import { Modal, Stagger, StaggerItem } from "../user-interface";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Textarea } from "@/components/ui/textarea";
import { createPlaybook, updatePlaybook, type Playbook } from "../../backend";

type FormValues = {
  name: string;
  edgeName: string;
  entryRules: string;
  exitRules: string;
  positionSizingRules: string;
  additionalRules: string;
};

function initialValues(p?: Playbook): FormValues {
  return {
    name: p?.name ?? "",
    edgeName: p?.edgeName ?? "",
    entryRules: p?.entryRules ?? "",
    exitRules: p?.exitRules ?? "",
    positionSizingRules: p?.positionSizingRules ?? "",
    additionalRules: p?.additionalRules ?? "",
  };
}

export function PlaybookFormDialog({
  playbook,
  trigger,
  onSaved,
}: {
  playbook?: Playbook;
  trigger: ReactNode;
  onSaved: () => void;
}) {
  return (
    <Modal
      trigger={trigger}
      icon={<BookOpenIcon size={16} weight="bold" />}
      title={playbook ? "Edit playbook" : "New playbook"}
      description={
        playbook
          ? "Update this playbook's edge and rules."
          : "Define a repeatable setup with its edge and rules."
      }
      size="md"
    >
      {(close) => (
        <PlaybookForm playbook={playbook} onSaved={onSaved} close={close} />
      )}
    </Modal>
  );
}

function PlaybookForm({
  playbook,
  onSaved,
  close,
}: {
  playbook?: Playbook;
  onSaved: () => void;
  close: () => void;
}) {
  const [values, setValues] = useState<FormValues>(() =>
    initialValues(playbook),
  );
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const set = (key: keyof FormValues) => (v: string) =>
    setValues((s) => ({ ...s, [key]: v }));

  const valid =
    values.name.trim() &&
    values.edgeName.trim() &&
    values.entryRules.trim() &&
    values.exitRules.trim() &&
    values.positionSizingRules.trim();

  async function submit() {
    if (!valid || saving) return;
    setSaving(true);
    setError(null);
    const additional = values.additionalRules.trim() || null;
    try {
      if (playbook) {
        await updatePlaybook(playbook.id, {
          name: values.name,
          edgeName: values.edgeName,
          entryRules: values.entryRules,
          exitRules: values.exitRules,
          positionSizingRules: values.positionSizingRules,
          additionalRules: additional,
          clearAdditionalRules: additional === null,
        });
      } else {
        await createPlaybook({
          name: values.name,
          edgeName: values.edgeName,
          entryRules: values.entryRules,
          exitRules: values.exitRules,
          positionSizingRules: values.positionSizingRules,
          additionalRules: additional,
        });
      }
      onSaved();
      close();
    } catch (e) {
      setError(String(e));
    } finally {
      setSaving(false);
    }
  }

  return (
    <form
      onSubmit={(e) => {
        e.preventDefault();
        submit();
      }}
      className="flex flex-col gap-4"
    >
      <Stagger className="flex flex-col gap-4">
        <StaggerItem className="grid grid-cols-2 gap-3">
          <div className="flex flex-col gap-2">
            <Label>Name</Label>
            <Input
              value={values.name}
              onChange={(e) => set("name")(e.target.value)}
              placeholder="Breakout"
              className="bg-muted/50"
            />
          </div>
          <div className="flex flex-col gap-2">
            <Label>Edge</Label>
            <Input
              value={values.edgeName}
              onChange={(e) => set("edgeName")(e.target.value)}
              placeholder="Momentum"
              className="bg-muted/50"
            />
          </div>
        </StaggerItem>

        {(
          [
            ["entryRules", "Entry rules"],
            ["exitRules", "Exit rules"],
            ["positionSizingRules", "Position sizing rules"],
            ["additionalRules", "Additional rules (optional)"],
          ] as const
        ).map(([key, label]) => (
          <StaggerItem key={key} className="flex flex-col gap-2">
            <Label>{label}</Label>
            <Textarea
              className="min-h-16 resize-y bg-muted/50"
              value={values[key]}
              onChange={(e) => set(key)(e.target.value)}
            />
          </StaggerItem>
        ))}
      </Stagger>

      {error && (
        <p className="text-sm text-red-600 dark:text-red-400">{error}</p>
      )}

      <div className="-mx-4 -mb-4 mt-1 flex items-center justify-end gap-2 rounded-b-2xl border-t bg-muted/50 p-4">
        <Button type="button" variant="ghost" onClick={close}>
          Cancel
        </Button>
        <Button type="submit" disabled={!valid || saving}>
          {saving ? "Saving…" : "Save"}
        </Button>
      </div>
    </form>
  );
}
