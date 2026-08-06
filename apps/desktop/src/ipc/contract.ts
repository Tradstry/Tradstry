export type DesktopEvent<T = unknown> = {
  event: string;
  payload: T;
};

export type Unlisten = () => void;

export interface DesktopBridge {
  invoke<T>(command: string, args?: Record<string, unknown>): Promise<T>;
  listen<T>(event: string, listener: (event: DesktopEvent<T>) => void): Unlisten;
  mediaUrl(path: string): string;
}

declare global {
  interface Window {
    tradstry: DesktopBridge;
  }
}
