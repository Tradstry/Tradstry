export type DesktopEvent<T = unknown> = {
  event: string;
  payload: T;
};

export type Unlisten = () => void;

export interface DesktopBridge {
  invoke<T>(command: string, args?: Record<string, unknown>): Promise<T>;
  listen<T>(event: string, listener: (event: DesktopEvent<T>) => void): Unlisten;
  mediaUrl(path: string): string;
  openExternal(url: string): Promise<void>;
  subscribe<T>(
    query: string,
    variables: Record<string, unknown> | undefined,
    handlers: {
      onMessage: (data: T) => void;
      onError?: (error: Error) => void;
      onComplete?: () => void;
    },
  ): Unlisten;
}

declare global {
  interface Window {
    tradstry: DesktopBridge;
  }
}
