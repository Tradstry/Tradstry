import { useEffect, useState, type ReactNode } from "react";
import { PencilSimpleIcon, PlusIcon } from "@phosphor-icons/react";
import {
  DateTimePicker,
  Modal,
  NumberField,
  Select,
  Stagger,
  StaggerItem,
} from "../user-interface";
import { TagPicker } from "./tag-picker";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Textarea } from "@/components/ui/textarea";
import { cn } from "@/lib/utils";
import {
  createJournalEntry,
  createTag,
  playbooks as fetchPlaybooks,
  tagCategories as fetchTagCategories,
  tags as fetchTags,
  updateJournalEntry,
  type JournalEntry,
  type Playbook,
  type Tag,
  type TagCategory,
} from "../../backend";

type FormValues = {
  symbol: string;
  symbolName: string;
  tradeType: "long" | "short";
  openDate: string;
  closeDate: string;
  entryPrice: string;
  exitPrice: string;
  positionSize: string;
  stopLoss: string;
  playbookId: string;
  notes: string;
};

function toDatetimeLocal(s: string): string {
  if (!s) return "";
  if (s.includes("T")) return s.slice(0, 16);
  if (/^\d{4}-\d{2}-\d{2}$/.test(s)) return `${s}T00:00`;
  const d = new Date(s);
  if (Number.isNaN(d.getTime())) return "";
  const pad = (n: number) => String(n).padStart(2, "0");
  return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}T${pad(d.getHours())}:${pad(d.getMinutes())}`;
}

function initialValues(entry?: JournalEntry): FormValues {
  return {
    symbol: entry?.symbol ?? "",
    symbolName: entry?.symbolName ?? "",
    tradeType: entry?.tradeType ?? "long",
    openDate: toDatetimeLocal(entry?.openDate ?? ""),
    closeDate: toDatetimeLocal(entry?.closeDate ?? ""),
    entryPrice: entry ? String(entry.entryPrice) : "",
    exitPrice: entry ? String(entry.exitPrice) : "",
    positionSize: entry ? String(entry.positionSize) : "",
    stopLoss: entry ? String(entry.stopLoss) : "",
    playbookId: entry?.playbookId ?? "",
    notes: entry?.notes ?? "",
  };
}

function Field({
  label,
  className,
  children,
}: {
  label: string;
  className?: string;
  children: ReactNode;
}) {
  return (
    <div className={cn("grid gap-2", className)}>
      <Label>{label}</Label>
      {children}
    </div>
  );
}

export function TradeFormDialog({
  entry,
  accountId,
  trigger,
  onSaved,
}: {
  entry?: JournalEntry;
  accountId: string | null;
  trigger: ReactNode;
  onSaved: () => void;
}) {
  return (
    <Modal
      trigger={trigger}
      icon={
        entry ? (
          <PencilSimpleIcon size={16} weight="bold" />
        ) : (
          <PlusIcon size={16} weight="bold" />
        )
      }
      title={entry ? "Edit trade" : "Enter trade"}
      description={
        entry
          ? "Update the details for this trade."
          : "Add a journal entry with the trade details."
      }
      size="xl"
    >
      {(close) => (
        <TradeForm
          entry={entry}
          accountId={accountId}
          onSaved={onSaved}
          close={close}
        />
      )}
    </Modal>
  );
}

function TradeForm({
  entry,
  accountId,
  onSaved,
  close,
}: {
  entry?: JournalEntry;
  accountId: string | null;
  onSaved: () => void;
  close: () => void;
}) {
  const [values, setValues] = useState<FormValues>(() => initialValues(entry));
  const [selectedTagIds, setSelectedTagIds] = useState<string[]>(
    () => entry?.tags.map((t) => t.id) ?? [],
  );
  const [allTags, setAllTags] = useState<Tag[]>([]);
  const [categories, setCategories] = useState<TagCategory[]>([]);
  const [allPlaybooks, setAllPlaybooks] = useState<Playbook[]>([]);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    fetchTags().then(setAllTags).catch(() => {});
    fetchTagCategories().then(setCategories).catch(() => {});
    fetchPlaybooks().then(setAllPlaybooks).catch(() => {});
  }, []);

  const set = (key: keyof FormValues) => (v: string) =>
    setValues((s) => ({ ...s, [key]: v }));

  /** The category a selected tag id belongs to (looked up from the tag list). */
  const catOf = (id: string) =>
    allTags.find((t) => t.id === id)?.categoryId ?? null;

  /** Replace this category's selection with `ids`, leaving other categories. */
  const setCategorySelection = (categoryId: string, ids: string[]) =>
    setSelectedTagIds((prev) => [
      ...prev.filter((id) => catOf(id) !== categoryId),
      ...ids,
    ]);

  const handleCreateTag = async (categoryId: string, name: string) => {
    const tag = await createTag(categoryId, name);
    setAllTags((prev) => [...prev, tag]);
    return tag;
  };

  const valid =
    values.symbol.trim() &&
    values.openDate &&
    values.closeDate &&
    values.entryPrice.trim() &&
    values.exitPrice.trim() &&
    values.positionSize.trim() &&
    values.stopLoss.trim();

  async function submit() {
    if (!valid || saving) return;
    if (!entry && !accountId) {
      setError("No account available to attach this trade to.");
      return;
    }
    setSaving(true);
    setError(null);
    const notes = values.notes.trim() || null;
    const playbookId = values.playbookId || null;
    const common = {
      openDate: values.openDate,
      closeDate: values.closeDate,
      entryPrice: Number(values.entryPrice),
      exitPrice: Number(values.exitPrice),
      positionSize: Number(values.positionSize),
      stopLoss: Number(values.stopLoss),
      symbol: values.symbol.trim().toUpperCase(),
      symbolName: values.symbolName.trim() || null,
      tradeType: values.tradeType,
      tagIds: selectedTagIds,
    };
    try {
      if (entry) {
        await updateJournalEntry(entry.id, {
          ...common,
          playbookId,
          notes,
          clearNotes: notes === null,
          clearPlaybook: playbookId === null,
        });
      } else {
        await createJournalEntry({
          accountId: accountId as string,
          ...common,
          playbookId: playbookId ?? undefined,
          notes: notes ?? undefined,
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
      <Stagger className="grid gap-4 md:grid-cols-2">
        <StaggerItem>
          <Field label="Symbol">
            <Input
              value={values.symbol}
              onChange={(e) => set("symbol")(e.target.value)}
              placeholder="AAPL"
              className="bg-muted/50"
            />
          </Field>
        </StaggerItem>
        <StaggerItem>
          <Field label="Symbol Name">
            <Input
              value={values.symbolName}
              onChange={(e) => set("symbolName")(e.target.value)}
              placeholder="Optional, auto-fetched if omitted"
              className="bg-muted/50"
            />
          </Field>
        </StaggerItem>

        <StaggerItem>
          <DateTimePicker
            label="Open Date"
            value={values.openDate}
            onChange={set("openDate")}
          />
        </StaggerItem>
        <StaggerItem>
          <DateTimePicker
            label="Close Date"
            value={values.closeDate}
            onChange={set("closeDate")}
          />
        </StaggerItem>

        <StaggerItem>
          <NumberField
            label="Entry Price"
            prefix="$"
            placeholder="0.00"
            value={values.entryPrice}
            onChange={set("entryPrice")}
          />
        </StaggerItem>
        <StaggerItem>
          <NumberField
            label="Exit Price"
            prefix="$"
            placeholder="0.00"
            value={values.exitPrice}
            onChange={set("exitPrice")}
          />
        </StaggerItem>

        <StaggerItem>
          <NumberField
            label="Position Size"
            suffix="shares"
            placeholder="1000.00"
            value={values.positionSize}
            onChange={set("positionSize")}
          />
        </StaggerItem>
        <StaggerItem>
          <NumberField
            label="Stop Loss"
            prefix="$"
            placeholder="0.00"
            value={values.stopLoss}
            onChange={set("stopLoss")}
          />
        </StaggerItem>

        <StaggerItem>
          <Select
            label="Trade Type"
            value={values.tradeType}
            onChange={(v) => set("tradeType")(v)}
            options={[
              { id: "long", label: "Long" },
              { id: "short", label: "Short" },
            ]}
          />
        </StaggerItem>
        <StaggerItem>
          <Select
            label="Playbook (Optional)"
            value={values.playbookId || "none"}
            onChange={(v) => set("playbookId")(v === "none" ? "" : v)}
            placeholder="No playbook"
            options={[
              { id: "none", label: "No playbook" },
              ...allPlaybooks.map((p) => ({ id: p.id, label: p.name })),
            ]}
          />
        </StaggerItem>

        <StaggerItem className="md:col-span-2">
          <Field label="Notes">
            <Textarea
              value={values.notes}
              onChange={(e) => set("notes")(e.target.value)}
              placeholder="Optional notes"
              rows={4}
              className="min-h-24 bg-muted/50"
            />
          </Field>
        </StaggerItem>

        {categories.map((cat) => (
          <StaggerItem key={cat.id}>
            <Field label={cat.name}>
              <TagPicker
                category={cat}
                tags={allTags}
                selectedTagIds={selectedTagIds.filter(
                  (id) => catOf(id) === cat.id,
                )}
                onChange={(ids) => setCategorySelection(cat.id, ids)}
                onCreate={(name) => handleCreateTag(cat.id, name)}
              />
            </Field>
          </StaggerItem>
        ))}
      </Stagger>

      {error && <p className="text-sm text-destructive">{error}</p>}

      <div className="-mx-4 -mb-4 mt-1 flex items-center justify-end gap-2 rounded-b-2xl border-t bg-muted/50 p-4">
        <Button type="button" variant="ghost" onClick={close}>
          Cancel
        </Button>
        <Button type="submit" disabled={!valid || saving}>
          {saving ? "Saving…" : "Save Trade"}
        </Button>
      </div>
    </form>
  );
}
