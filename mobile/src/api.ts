import { Capacitor, registerPlugin } from "@capacitor/core";

export interface BackendConnection { baseUrl: string; token: string; }
export interface ProviderOption { id: string; name: string; available: boolean; capabilities: Record<string, unknown>; }
export interface ProviderMediaId { provider: string; value: string; }
export interface CatalogItem {
  id: ProviderMediaId;
  title: string;
  media_type: "movie" | "series";
  year?: string | null;
  poster_url?: string | null;
  season_count?: number | null;
  /** Same title found on other providers/audio cuts. */
  sources?: ProviderMediaId[];
  languages?: string[];
  provider_count?: number;
}
export interface MobileHomepage {
  items: CatalogItem[];
  metrics: Record<string, unknown>;
}
export interface PlaybackMirror {
  label: string;
  resolver_url: string;
  headers: Array<[string, string]>;
  direct_file: boolean;
}
export interface PlaybackRelease {
  provider: string;
  filename: string;
  quality?: string | null;
  codec?: string | null;
  language?: string | null;
  size_bytes?: number | null;
  season?: number | null;
  episode?: number | null;
  mirrors: PlaybackMirror[];
  resource_id?: string | null;
}
export interface SubtitleOption { name: string; url: string; }
export interface MobilePlaybackResponse {
  provider: string;
  media_id: string;
  season: number;
  episode: number;
  releases: PlaybackRelease[];
  subtitles: SubtitleOption[];
  provider_sources?: ProviderMediaId[];
  release_sources?: Array<{ provider: string; media_id: string; release_index: number }>;
}
export interface ResolvedPlaybackSource {
  provider: string;
  url: string;
  headers: Array<[string, string]>;
  subtitle?: string | null;
  source_label: string;
  max_height?: number | null;
}
export interface NativeStreamSource {
  url: string;
  headers: Record<string, string>;
  subtitleUrl?: string;
  label?: string;
}
export interface NativePlayerOptions {
  url: string;
  title: string;
  headers: Record<string, string>;
  subtitleUrl?: string;
  sources?: NativeStreamSource[];
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
interface NativeBackendPlugin { start(): Promise<BackendConnection>; stop(): Promise<void>; playVideo(options: NativePlayerOptions): Promise<void>; }

const NativeBackend = registerPlugin<NativeBackendPlugin>("MobileBackend");
export async function playInNativePlayer(options: NativePlayerOptions): Promise<void> {
  if (!Capacitor.isNativePlatform()) {
    throw new Error("In-app playback is available in the installed Android app.");
  }
  await NativeBackend.playVideo(options);
}

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
