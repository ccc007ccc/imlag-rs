import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type { UiEventDto } from "./types";

// Subscribe to the engine's UI event stream. Returns a Promise<UnlistenFn>
// because Tauri's listen registers asynchronously — keep the promise's
// resolved value so the cleanup hook can call it.
export function onUiEvent(cb: (e: UiEventDto) => void): Promise<UnlistenFn> {
  return listen<UiEventDto>("ui-event", (raw) => cb(raw.payload));
}
