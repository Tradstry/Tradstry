import type { DesktopDatabase } from "./database.ts";
import { transaction } from "./database.ts";
import { enqueueMutation, uuidV7 } from "./mutations.ts";

export type PrincipleInput = {
  accountId: string;
  playbookId?: string | null;
  evidenceNoteId?: string | null;
  title: string;
  theRule?: string;
  why?: string;
  intervention?: string | null;
  isActive?: boolean;
  clearIntervention?: boolean;
  clearPlaybook?: boolean;
  clearEvidenceNote?: boolean;
};

export type Principle = {
  id: string;
  accountId: string;
  playbookId: string | null;
  evidenceNoteId: string | null;
  evidenceNoteTitle: string | null;
  title: string;
  theRule: string;
  why: string;
  intervention: string | null;
  priority: number;
  isActive: boolean;
  createdAt: string;
  violationCount: number;
  violatedCumulativeProfit: number;
  violatedCumulativeRoi: number;
  violatedWinRate: number;
};

type PrincipleRow = {
  id: string;
  account_id: string;
  playbook_id: string | null;
  evidence_note_id: string | null;
  title: string;
  the_rule: string;
  why: string;
  intervention: string | null;
  priority: number;
  is_active: number;
  created_at: string;
};

type PrincipleWrite = {
  id: string;
  accountId: string;
  playbookId: string | null;
  evidenceNoteId: string | null;
  title: string;
  theRule: string;
  why: string;
  intervention: string | null;
  priority: number;
  isActive: boolean;
};

const COLUMNS = "id, account_id, playbook_id, evidence_note_id, title, the_rule, why, intervention, priority, is_active, created_at";

export class PrinciplesRepository {
  readonly #store: DesktopDatabase;

  constructor(store: DesktopDatabase) {
    this.#store = store;
  }

  principles(accountId: string): Principle[] {
    const rows = this.#store.db
      .prepare(`SELECT ${COLUMNS} FROM trading_principles WHERE account_id = ? AND deleted_at IS NULL ORDER BY priority DESC`)
      .all(accountId) as PrincipleRow[];
    return rows.map((row) => this.#view(row));
  }

  create(input: PrincipleInput): Principle {
    if (!input.accountId) throw new Error("accountId is required");
    if (!input.title) throw new Error("title is required");
    const write: PrincipleWrite = {
      id: uuidV7(),
      accountId: input.accountId,
      playbookId: input.playbookId ?? null,
      evidenceNoteId: input.evidenceNoteId ?? null,
      title: input.title,
      theRule: input.theRule ?? "",
      why: input.why ?? "",
      intervention: input.intervention ?? null,
      priority: 0,
      isActive: input.isActive ?? true,
    };
    this.#write("create", write);
    return this.#view(this.#required(write.id));
  }

  update(id: string, input: Partial<PrincipleInput>): Principle {
    const current = this.#required(id);
    const write: PrincipleWrite = {
      id,
      accountId: current.account_id,
      playbookId: input.clearPlaybook ? null : input.playbookId ?? current.playbook_id,
      evidenceNoteId: input.clearEvidenceNote ? null : input.evidenceNoteId ?? current.evidence_note_id,
      title: input.title ?? current.title,
      theRule: input.theRule ?? current.the_rule,
      why: input.why ?? current.why,
      intervention: input.clearIntervention ? null : input.intervention ?? current.intervention,
      priority: current.priority,
      isActive: input.isActive ?? Boolean(current.is_active),
    };
    this.#write("update", write);
    return this.#view(this.#required(id));
  }

  delete(id: string): boolean {
    const stamp = this.#store.hlc.now();
    transaction(this.#store.db, () => {
      this.#store.db
        .prepare("UPDATE trading_principles SET deleted_at = datetime('now'), hlc = ?, sync_state = 'pending' WHERE id = ?")
        .run(stamp, id);
      enqueueMutation(this.#store.db, "deletePrinciple", { id }, stamp);
    });
    return true;
  }

  reorder(orderedIds: string[]): boolean {
    const stamp = this.#store.hlc.now();
    transaction(this.#store.db, () => {
      const top = orderedIds.length;
      orderedIds.forEach((id, index) => {
        this.#store.db
          .prepare("UPDATE trading_principles SET priority = ?, hlc = ?, sync_state = 'pending' WHERE id = ?")
          .run(top - index, stamp, id);
      });
      enqueueMutation(this.#store.db, "reorderPrinciples", { orderedIds }, stamp);
    });
    return true;
  }

  #write(kind: "create" | "update", row: PrincipleWrite): void {
    const stamp = this.#store.hlc.now();
    transaction(this.#store.db, () => {
      if (kind === "create") {
        this.#store.db
          .prepare(
            `INSERT INTO trading_principles
             (id, account_id, playbook_id, evidence_note_id, title, the_rule, why, intervention,
              priority, is_active, hlc, sync_state)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 'pending')`,
          )
          .run(row.id, row.accountId, row.playbookId, row.evidenceNoteId, row.title, row.theRule, row.why, row.intervention, row.priority, Number(row.isActive), stamp);
      } else {
        this.#store.db
          .prepare(
            `UPDATE trading_principles SET playbook_id = ?, evidence_note_id = ?, title = ?,
             the_rule = ?, why = ?, intervention = ?, priority = ?, is_active = ?, hlc = ?,
             sync_state = 'pending' WHERE id = ?`,
          )
          .run(row.playbookId, row.evidenceNoteId, row.title, row.theRule, row.why, row.intervention, row.priority, Number(row.isActive), stamp, row.id);
      }
      enqueueMutation(this.#store.db, kind === "create" ? "createPrinciple" : "updatePrinciple", {
        id: row.id,
        accountId: row.accountId,
        playbookId: row.playbookId,
        evidenceNoteId: row.evidenceNoteId,
        title: row.title,
        theRule: row.theRule,
        why: row.why,
        intervention: row.intervention,
        isActive: row.isActive,
        priority: row.priority,
      }, stamp);
    });
  }

  #required(id: string): PrincipleRow {
    const row = this.#store.db
      .prepare(`SELECT ${COLUMNS} FROM trading_principles WHERE id = ? AND deleted_at IS NULL`)
      .get(id) as PrincipleRow | undefined;
    if (!row) throw new Error("principle not found");
    return row;
  }

  #view(row: PrincipleRow): Principle {
    const evidence = row.evidence_note_id
      ? (this.#store.db.prepare("SELECT title FROM notes WHERE id = ? AND deleted_at IS NULL").get(row.evidence_note_id) as
          | { title: string }
          | undefined)
      : undefined;
    const entries = this.#store.db
      .prepare(
        `SELECT entry_price, position_size, total_pl, violated_principle_ids
         FROM journal_entries WHERE account_id = ? AND deleted_at IS NULL`,
      )
      .all(row.account_id) as Array<{
      entry_price: number;
      position_size: number;
      total_pl: number;
      violated_principle_ids: string;
    }>;
    const violators = entries.filter((entry) => stringArray(entry.violated_principle_ids).includes(row.id));
    const winners = violators.filter((entry) => entry.total_pl > 0).length;
    const losers = violators.filter((entry) => entry.total_pl < 0).length;
    const decisive = winners + losers;
    return {
      id: row.id,
      accountId: row.account_id,
      playbookId: row.playbook_id,
      evidenceNoteId: row.evidence_note_id,
      evidenceNoteTitle: evidence?.title ?? null,
      title: row.title,
      theRule: row.the_rule,
      why: row.why,
      intervention: row.intervention,
      priority: row.priority,
      isActive: Boolean(row.is_active),
      createdAt: row.created_at,
      violationCount: violators.length,
      violatedCumulativeProfit: violators.reduce(
        (sum, entry) => sum + (entry.position_size * entry.entry_price * entry.total_pl) / 100,
        0,
      ),
      violatedCumulativeRoi: violators.reduce((sum, entry) => sum + entry.total_pl, 0),
      violatedWinRate: decisive > 0 ? (winners / decisive) * 100 : 0,
    };
  }
}

function stringArray(value: string): string[] {
  try {
    const parsed: unknown = JSON.parse(value);
    return Array.isArray(parsed) ? parsed.filter((item): item is string => typeof item === "string") : [];
  } catch {
    return [];
  }
}
