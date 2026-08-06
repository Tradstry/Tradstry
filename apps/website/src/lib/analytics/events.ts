import { Countly, countlyEnabled } from "./countly";

export { EVENTS, type EventName, type EventProps } from "@tradstry/app-ui/lib/analytics/events";
import type { EventName, EventProps } from "@tradstry/app-ui/lib/analytics/events";

export function capture<K extends EventName>(event: K, props: EventProps[K]) {
  if (countlyEnabled()) {
    Countly.add_event({ key: event, count: 1, segmentation: props });
  }
}
