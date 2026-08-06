const COUNTER_MAX = 0xffff;

export type PhysicalClock = () => number;

/** Lexicographically sortable hybrid logical clock used by desktop mutations. */
export class Hlc {
  #millis = 0;
  #counter = 0;
  readonly clientId: string;
  private readonly physical: PhysicalClock;

  constructor(clientId: string, physical: PhysicalClock = Date.now) {
    this.clientId = clientId;
    this.physical = physical;
  }

  now(): string {
    const physical = Math.max(0, Math.trunc(this.physical()));
    if (physical > this.#millis) {
      this.#millis = physical;
      this.#counter = 0;
    } else {
      this.#counter = Math.min(COUNTER_MAX, this.#counter + 1);
      if (this.#counter === COUNTER_MAX) {
        throw new Error("HLC counter saturated");
      }
    }
    return `${String(this.#millis).padStart(15, "0")}:${String(this.#counter).padStart(5, "0")}:${this.clientId}`;
  }

  observe(remote: string): void {
    const parsed = parseStamp(remote);
    if (!parsed) return;
    if (parsed.millis > this.#millis) {
      this.#millis = parsed.millis;
      this.#counter = parsed.counter;
    } else if (parsed.millis === this.#millis && parsed.counter > this.#counter) {
      this.#counter = parsed.counter;
    }
  }
}

export function parseStamp(stamp: string): { millis: number; counter: number } | null {
  const [millisValue, counterValue] = stamp.split(":", 3);
  const millis = Number(millisValue);
  const counter = Number(counterValue);
  if (!Number.isSafeInteger(millis) || !Number.isInteger(counter) || counter < 0 || counter > COUNTER_MAX) {
    return null;
  }
  return { millis, counter };
}
