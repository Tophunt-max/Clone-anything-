// The Android app's bridge, present only when this page runs inside the app's WebView on the
// trusted origin (the app injects `forgeNative` nowhere else). Requests are JSON with an id;
// the app answers with `{id, ok, result?, error?}`.

interface NativePort {
  postMessage(message: string): void;
  addEventListener(type: "message", listener: (e: MessageEvent) => void): void;
  removeEventListener(type: "message", listener: (e: MessageEvent) => void): void;
}

export interface NativeReply {
  id: string;
  ok: boolean;
  result?: Record<string, string>;
  error?: string;
}

function port(): NativePort | undefined {
  const p = (globalThis as { forgeNative?: NativePort }).forgeNative;
  return p && typeof p.postMessage === "function" ? p : undefined;
}

export function inAndroidApp(): boolean {
  return port() !== undefined;
}

let counter = 0;

/** Sends one request; resolves with the app's reply, or rejects after `timeoutMs`. */
export function callNative(method: "getAppInfo" | "haptic" | "share" | "openExternal", params?: Record<string, string>, timeoutMs = 3000): Promise<NativeReply> {
  const p = port();
  if (!p) return Promise.reject(new Error("Not running in the Android app."));
  const id = `w${Date.now().toString(36)}${(counter++).toString(36)}`;
  return new Promise((resolve, reject) => {
    const done = (fn: () => void) => {
      clearTimeout(timer);
      p.removeEventListener("message", onMessage);
      fn();
    };
    const onMessage = (e: MessageEvent) => {
      let reply: NativeReply;
      try {
        reply = JSON.parse(String(e.data)) as NativeReply;
      } catch {
        return;
      }
      if (reply.id === id) done(() => resolve(reply));
    };
    const timer = setTimeout(() => done(() => reject(new Error("The app didn't answer."))), timeoutMs);
    p.addEventListener("message", onMessage);
    p.postMessage(JSON.stringify(params ? { id, method, params } : { id, method }));
  });
}
