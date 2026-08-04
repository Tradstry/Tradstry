import { Countly, countlyEnabled } from "@/lib/analytics/countly";

export const EVENTS = {
  ctaClicked: "cta_clicked",
  pricingViewed: "pricing_viewed",
  signedUp: "signed_up",
  signedIn: "signed_in",

  brokerageConnectStarted: "brokerage_connect_started",
  brokerageConnected: "brokerage_connected",
  tradesMerged: "trades_merged",

  tradeLogged: "trade_logged",
  tradeEdited: "trade_edited",
  tradeDeleted: "trade_deleted",
  tagApplied: "tag_applied",
  playbookCreated: "playbook_created",
  principleCreated: "principle_created",
  violationFlagged: "violation_flagged",
  noteCreated: "note_created",
  noteEdited: "note_edited",
  chatMessageSent: "chat_message_sent",
  analyticsRangeChanged: "analytics_range_changed",

  dataExportRequested: "data_export_requested",
  accountDeletionRequested: "account_deletion_requested",
} as const;

export type EventName = (typeof EVENTS)[keyof typeof EVENTS];

export type EventProps = {
  cta_clicked: { location: string; label: string };
  pricing_viewed: Record<string, never>;
  signed_up: Record<string, never>;
  signed_in: Record<string, never>;

  brokerage_connect_started: Record<string, never>;
  brokerage_connected: { broker: string };
  trades_merged: { count: number };

  trade_logged: {
    accountId: string;
    symbol: string;
    source: "manual" | "broker";
  };
  trade_edited: { accountId: string };
  trade_deleted: { accountId: string };
  tag_applied: { tagId: string; role: string };
  playbook_created: Record<string, never>;
  principle_created: Record<string, never>;
  violation_flagged: { principleId: string };
  note_created: Record<string, never>;
  note_edited: Record<string, never>;
  chat_message_sent: { hasContext: boolean };
  analytics_range_changed: { range: string };

  data_export_requested: Record<string, never>;
  account_deletion_requested: Record<string, never>;
};

export function capture<K extends EventName>(event: K, props: EventProps[K]) {
  if (!countlyEnabled()) {
    return;
  }
  Countly.add_event({ key: event, count: 1, segmentation: props });
}
