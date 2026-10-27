import { Capacitor, registerPlugin } from "@capacitor/core";

export interface BackendConnection { baseUrl: string; token: string; }
export interface ProviderOption { id: string; name: string; available: boolean; capabilities: Record<string, unknown>; }
export interface CatalogItem {
  id: { provider: string; value: string };
  title: string;
  media_type: "movie" | "series";
  year?: string | null;
  poster_url?: string | null;
  season_count?: number | null;
}
export interface MediaDetails {
  id: { provider: string; value: string };
  title: string;
  media_type: "movie" | "series";
  year?: string | null;
  description?: string | null;
  tagline?: string | null;
  imdb_rating?: string | null;
  director?: string | null;
  stars?: string | null;
  poster_url?: string | null;
  duration?: string | null;
  genres: string[];
  seasons: Array<{ number: number; episodes: Array<{ season: number; number: number; title?: string | null; overview?: string | null }> }>;
  dubs: Array<{ subject_id: string; language: string; label: string }>;
}
interface NativeBackendPlugin { start(): Promise<BackendConnection>; stop(): Promise<void>; }

const NativeBackend = registerPlugin<NativeBackendPlugin>("MobileBackend");
let connectionPromise: Promise<BackendConnection> | null = null;
export function resetConnection(): void { connectionPromise = null; }

export function connectBackend(): Promise<BackendConnection> {
  if (!connectionPromise) {
    connectionPromise = (async () => {
      if (Capacitor.isNativePlatform()) return NativeBackend.start();
      const baseUrl = import.meta.env.VITE_API_BASE_URL;
      const token = import.meta.env.VITE_API_TOKEN;
      if (!baseUrl || !token) {
        throw new Error("The on-device service starts inside Android. For browser development, set VITE_API_BASE_URL and VITE_API_TOKEN.");
      }
      return { baseUrl: baseUrl.replace(/\/+$/, ""), token };
    })();
  }
  return connectionPromise;
}

export async function apiRequest<T>(path: string, init: RequestInit = {}): Promise<T> {
  const connection = await connectBackend();
  const headers = new Headers(init.headers);
  headers.set("X-7xCinema-Token", connection.token);
  if (init.body && !headers.has("Content-Type")) headers.set("Content-Type", "application/json");
  const response = await fetch(connection.baseUrl + path, { ...init, headers, cache: "no-store" });
  let payload: unknown = null;
  try { payload = await response.json(); } catch { payload = null; }
  if (!response.ok) {
    const error = typeof payload === "object" && payload !== null && "error" in payload
      ? (payload as { error?: unknown }).error : null;
    throw new Error(typeof error === "string" ? error : "Request failed (" + response.status + ")");
  }
  return payload as T;
}
