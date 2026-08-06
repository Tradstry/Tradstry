import { useEffect, useState, type ReactNode } from "react";
import { CheckCircleIcon } from "@phosphor-icons/react";
import { Modal, Stagger, StaggerItem } from "../../user-interface";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Textarea } from "@/components/ui/textarea";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import {
  createPrinciple,
  playbooks as fetchPlaybooks,
  updatePrinciple,
  type Playbook,
  type PrincipleWithStats,
} from "../../../backend";

const NO_PLAYBOOK = "__none__";

type FormValues = {
  title: string;
  theRule: string;
  why: string;
  intervention: string;
  playbookId: string;
  isActive: boolean;
};

function initialValues(p?: PrincipleWithStats): FormValues {
  return {
    title: p?.title ?? "",
    theRule: p?.theRule ?? "",
    why: p?.why ?? "",
    intervention: p?.intervention ?? "",
    playbookId: p?.playbookId ?? NO_PLAYBOOK,
    isActive: p?.isActive ?? true,
  };
}

export function PrincipleFormDialog({
  accountId,
  principle,
  trigger,
  onSaved,
}: {
  accountId: string;
  principle?: PrincipleWithStats;
  trigger: ReactNode;
  onSaved: () => void;
}) {
  return (
    <Modal
      trigger={trigger}
      icon={<CheckCircleIcon size={16} weight="bold" />}
      title={principle ? "Edit principle" : "New principle"}
      description={
        principle
          ? "Deactivating keeps its history but hides it from new trades."
          : "A rule you will read before trading, and tick when you break it."
      }
      size="md"
    >
      {(close) => (
        <PrincipleForm
          accountId={accountId}
          principle={principle}
          onSaved={onSaved}
          close={close}
        />
      )}
    </Modal>
  );
}

function PrincipleForm({
  accountId,
  principle,
  onSaved,
  close,
}: {
  accountId: string;
  principle?: PrincipleWithStats;
  onSaved: () => void;
  close: () => void;
}) {
  const [values, setValues] = useState<FormValues>(() =>
    initialValues(principle),
  );
  const [playbookOptions, setPlaybookOptions] = useState<Playbook[]>([]);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    fetchPlaybooks()
      .then(setPlaybookOptions)
      .catch(() => setPlaybookOptions([]));
  }, []);

  const set = <K extends keyof FormValues>(key: K) => (v: FormValues[K]) =>
    setValues((s) => ({ ...s, [key]: v }));

  const valid =
    values.title.trim() && values.theRule.trim() && values.why.trim();

  async function submit() {
    if (!valid || saving) return;
    setSaving(true);
    setError(null);
    const intervention = values.intervention.trim() || null;
    const playbookId =
      values.playbookId === NO_PLAYBOOK ? null : values.playbookId;
    try {
      if (principle) {
        await updatePrinciple(principle.id, {
          title: values.title,
          theRule: values.theRule,
          why: values.why,
          intervention,
          clearIntervention: intervention === null,
          playbookId,
          clearPlaybook: playbookId === null,
          isActive: values.isActive,
        });
      } else {
        await createPrinciple({
          accountId,
          title: values.title,
          theRule: values.theRule,
          why: values.why,
          intervention,
          playbookId,
          isActive: values.isActive,
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
        <StaggerItem className="flex flex-col gap-2">
          <Label>Title</Label>
          <Input
            value={values.title}
            onChange={(e) => set("title")(e.target.value)}
            placeholder="30-min rule"
            maxLength={80}
            className="bg-muted/50"
          />
        </StaggerItem>

        <StaggerItem className="flex flex-col gap-2">
          <Label>Applies to</Label>
          <Select
            value={values.playbookId}
            onValueChange={(v) => set("playbookId")(v)}
          >
            <SelectTrigger className="w-full bg-muted/50">
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              <SelectItem value={NO_PLAYBOOK}>
                Applies to every trade
              </SelectItem>
              {playbookOptions.map((p) => (
                <SelectItem key={p.id} value={p.id}>
                  {p.name}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
        </StaggerItem>

        <StaggerItem className="flex flex-col gap-2">
          <Label>The rule</Label>
          <Textarea
            className="min-h-16 resize-y bg-muted/50"
            value={values.theRule}
            onChange={(e) => set("theRule")(e.target.value)}
            placeholder="Do not touch a position between 9:30 and 10:00 ET."
          />
        </StaggerItem>

        <StaggerItem className="flex flex-col gap-2">
          <Label>Why it matters</Label>
          <Textarea
            className="min-h-20 resize-y bg-muted/50"
            value={values.why}
            onChange={(e) => set("why")(e.target.value)}
            placeholder="The evidence. What breaking it has cost you."
          />
        </StaggerItem>

        <StaggerItem className="flex flex-col gap-2">
          <Label>Intervention (optional)</Label>
          <Textarea
            className="min-h-16 resize-y bg-muted/50"
            value={values.intervention}
            onChange={(e) => set("intervention")(e.target.value)}
            placeholder="The physical change that enforces the rule."
          />
        </StaggerItem>

        {principle ? (
          <StaggerItem className="flex items-center justify-between rounded-lg border border-input bg-muted/50 px-3 py-2.5">
            <div>
              <p className="text-sm font-medium text-foreground">
                Active (shown when logging trades)
              </p>
            </div>
            <Button
              type="button"
              variant={values.isActive ? "default" : "outline"}
              size="sm"
              onClick={() => set("isActive")(!values.isActive)}
            >
              {values.isActive ? "Active" : "Inactive"}
            </Button>
          </StaggerItem>
        ) : null}
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
