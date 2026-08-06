"use client";

import {
  closestCenter,
  DndContext,
  type DragEndEvent,
  PointerSensor,
  useSensor,
  useSensors,
} from "@dnd-kit/core";
import { restrictToVerticalAxis } from "@dnd-kit/modifiers";
import {
  arrayMove,
  SortableContext,
  useSortable,
  verticalListSortingStrategy,
} from "@dnd-kit/sortable";
import { CSS } from "@dnd-kit/utilities";
import { Delete02Icon, PencilEdit01Icon } from "@hugeicons/core-free-icons";
import { HugeiconsIcon } from "@hugeicons/react";
import * as React from "react";
import { toast } from "sonner";
import { Button } from "@/components/ui/button";
import {
  Empty,
  EmptyDescription,
  EmptyHeader,
  EmptyTitle,
} from "@/components/ui/empty";
import { useActiveWorkspace } from "@/components/workspaces/hooks";
import { usePlaybooks } from "@/hooks/playbook";
import {
  useDeletePrinciple,
  usePrinciples,
  useReorderPrinciples,
} from "@/hooks/principle";
import type { PrincipleWithStats } from "@/lib/types/principle";
import { cn } from "@/lib/utils";
import { CreatePrincipleDialog } from "./create-principle";
import { EditPrincipleDialog } from "./edit-principle";

const GLOBAL_GROUP = "__global__";

function formatBreaks(p: PrincipleWithStats) {
  const roi = p.violatedCumulativeRoi;
  const sign = roi > 0 ? "+" : "";
  return `${p.violationCount} ${p.violationCount === 1 ? "break" : "breaks"} · ${sign}${roi.toFixed(1)}%`;
}

type Group = { key: string; label: string; items: PrincipleWithStats[] };

/** Workspace-wide principles first, then one group per playbook that owns any. */
function groupPrinciples(
  principles: PrincipleWithStats[],
  playbookNames: Map<string, string>,
): Group[] {
  const globals = principles.filter((p) => p.playbookId === null);
  const groups: Group[] = [];

  if (globals.length > 0) {
    groups.push({
      key: GLOBAL_GROUP,
      label: "Applies to every trade in this workspace",
      items: globals,
    });
  }

  for (const [playbookId, name] of playbookNames) {
    const items = principles.filter((p) => p.playbookId === playbookId);
    if (items.length > 0) {
      groups.push({ key: playbookId, label: name, items });
    }
  }

  return groups;
}

function PrincipleRow({
  principle,
  rank,
  onEdit,
  onDelete,
}: {
  principle: PrincipleWithStats;
  rank: number;
  onEdit: () => void;
  onDelete: () => void;
}) {
  const [expanded, setExpanded] = React.useState(false);
  const {
    attributes,
    listeners,
    setNodeRef,
    transform,
    transition,
    isDragging,
  } = useSortable({ id: principle.id });

  const negative = principle.violatedCumulativeRoi < 0;
  const untouched = principle.violationCount === 0;

  return (
    <div
      ref={setNodeRef}
      style={{ transform: CSS.Transform.toString(transform), transition }}
      className={cn(
        "rounded-md border border-border/60 bg-card px-3 py-2",
        isDragging && "opacity-60",
      )}
    >
      <div className="flex items-center gap-2">
        <button
          type="button"
          aria-label={`Reorder ${principle.title}`}
          className="cursor-grab text-muted-foreground/60 hover:text-foreground"
          {...attributes}
          {...listeners}
        >
          ⠿
        </button>
        <span className="w-4 text-xs tabular-nums text-muted-foreground">
          {rank}
        </span>
        <button
          type="button"
          className="flex-1 text-left text-sm font-medium"
          onClick={() => setExpanded((v) => !v)}
          aria-expanded={expanded}
        >
          {principle.title}
        </button>
        <span
          className={cn(
            "text-xs tabular-nums",
            untouched
              ? "text-muted-foreground"
              : negative
                ? "text-destructive"
                : "text-foreground",
          )}
        >
          {formatBreaks(principle)}
        </span>
        <Button type="button" size="icon" variant="ghost" onClick={onEdit}>
          <HugeiconsIcon icon={PencilEdit01Icon} className="size-4" />
        </Button>
        <Button type="button" size="icon" variant="ghost" onClick={onDelete}>
          <HugeiconsIcon icon={Delete02Icon} className="size-4" />
        </Button>
      </div>

      <p className="pl-10 text-xs text-muted-foreground">{principle.theRule}</p>

      {expanded ? (
        <div className="space-y-2 pl-10 pt-2">
          <p className="text-xs">{principle.why}</p>
          {principle.intervention ? (
            <p className="text-xs">
              <span className="font-medium">Intervention: </span>
              {principle.intervention}
            </p>
          ) : null}
          {principle.evidenceNoteTitle ? (
            <a
              href={`/dashboard/notebook?note=${principle.evidenceNoteId}`}
              className="text-xs underline underline-offset-2"
            >
              📄 {principle.evidenceNoteTitle}
            </a>
          ) : null}
        </div>
      ) : null}
    </div>
  );
}

export function PrinciplesTab() {
  const activeWorkspace = useActiveWorkspace();
  const workspaceId = activeWorkspace?.id ?? null;

  const principlesQuery = usePrinciples(workspaceId);
  const playbooksQuery = usePlaybooks();
  const reorder = useReorderPrinciples(workspaceId ?? "");
  const deletePrinciple = useDeletePrinciple(workspaceId ?? "");

  const [showInactive, setShowInactive] = React.useState(false);
  const [editing, setEditing] = React.useState<PrincipleWithStats | null>(null);

  const sensors = useSensors(useSensor(PointerSensor));

  const all = principlesQuery.data ?? [];
  const playbooks = playbooksQuery.data ?? [];
  const playbookNames = React.useMemo(
    () => new Map(playbooks.map((p) => [p.id, p.name])),
    [playbooks],
  );

  const visible = showInactive ? all : all.filter((p) => p.isActive);
  const inactiveCount = all.filter((p) => !p.isActive).length;
  const groups = groupPrinciples(visible, playbookNames);

  async function handleDragEnd(event: DragEndEvent, group: Group) {
    const { active, over } = event;
    if (!over || active.id === over.id) return;

    const oldIndex = group.items.findIndex((p) => p.id === active.id);
    const newIndex = group.items.findIndex((p) => p.id === over.id);
    // `over` outside this group means a cross-group drag; ignore it. A global
    // principle must never become playbook-scoped by being dragged.
    if (oldIndex === -1 || newIndex === -1) return;

    const reorderedGroup = arrayMove(group.items, oldIndex, newIndex);

    // Send every principle id in its new global display order so priorities stay
    // monotonic across groups and never collide.
    const orderedIds = groups.flatMap((g) =>
      (g.key === group.key ? reorderedGroup : g.items).map((p) => p.id),
    );

    try {
      await reorder.mutateAsync(orderedIds);
    } catch (error) {
      toast.error(
        error instanceof Error
          ? error.message
          : "Failed to reorder principles.",
      );
    }
  }

  async function handleDelete(principle: PrincipleWithStats) {
    const toastId = toast.loading(`Deleting ${principle.title}...`);
    try {
      await deletePrinciple.mutateAsync(principle.id);
      toast.success("Principle deleted.", { id: toastId });
    } catch (error) {
      toast.error(
        error instanceof Error ? error.message : "Failed to delete principle.",
        { id: toastId },
      );
    }
  }

  if (!workspaceId) {
    return (
      <Empty>
        <EmptyHeader>
          <EmptyTitle>No workspace selected</EmptyTitle>
          <EmptyDescription>
            Create a workspace to start writing principles.
          </EmptyDescription>
        </EmptyHeader>
      </Empty>
    );
  }

  return (
    <div className="space-y-6">
      <div className="flex justify-end">
        <CreatePrincipleDialog
          workspaceId={workspaceId}
          playbooks={playbooks}
        />
      </div>

      {principlesQuery.isLoading ? (
        <p className="text-xs text-muted-foreground">Loading…</p>
      ) : groups.length === 0 ? (
        <Empty>
          <EmptyHeader>
            <EmptyTitle>No principles yet</EmptyTitle>
            <EmptyDescription>
              Write the rules you keep breaking. You will see what each one
              costs you.
            </EmptyDescription>
          </EmptyHeader>
        </Empty>
      ) : (
        groups.map((group) => (
          <section key={group.key} className="space-y-2">
            <h3 className="text-xs font-medium uppercase tracking-wide text-muted-foreground">
              {group.label}
            </h3>
            <DndContext
              sensors={sensors}
              collisionDetection={closestCenter}
              modifiers={[restrictToVerticalAxis]}
              onDragEnd={(event) => handleDragEnd(event, group)}
            >
              <SortableContext
                items={group.items.map((p) => p.id)}
                strategy={verticalListSortingStrategy}
              >
                <div className="space-y-2">
                  {group.items.map((principle, index) => (
                    <PrincipleRow
                      key={principle.id}
                      principle={principle}
                      rank={index + 1}
                      onEdit={() => setEditing(principle)}
                      onDelete={() => handleDelete(principle)}
                    />
                  ))}
                </div>
              </SortableContext>
            </DndContext>
          </section>
        ))
      )}

      {inactiveCount > 0 ? (
        <div className="flex justify-end">
          <Button
            type="button"
            size="sm"
            variant="ghost"
            onClick={() => setShowInactive((v) => !v)}
          >
            {showInactive
              ? "Hide inactive"
              : `Show inactive (${inactiveCount})`}
          </Button>
        </div>
      ) : null}

      {editing ? (
        <EditPrincipleDialog
          principle={editing}
          playbooks={playbooks}
          open={true}
          onOpenChange={(open) => {
            if (!open) setEditing(null);
          }}
        />
      ) : null}
    </div>
  );
}
