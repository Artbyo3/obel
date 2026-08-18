import { Track, Album, ViewMode } from "./types";
import { invoke } from "@tauri-apps/api/core";

let tracks: Track[] = [];
let isPlaying = false;
let currentView: ViewMode = 'tracks';
let currentAlbumContext: Album | null = null;
let playbackQueue: Track[] = [];
let currentQueueIndex = -1;
let searchQuery = "";
let currentTrack: Track | null = null;

export function getTracks() { return tracks; }
export function setTracks(t: Track[]) { tracks = t; }
export function getIsPlaying() { return isPlaying; }
export function setIsPlaying(v: boolean) { isPlaying = v; }
export function getCurrentView() { return currentView; }
export function setCurrentView(v: ViewMode) { currentView = v; }
export function getCurrentAlbumContext() { return currentAlbumContext; }
export function setCurrentAlbumContext(a: Album | null) { currentAlbumContext = a; }
export function getPlaybackQueue() { return playbackQueue; }
export function setPlaybackQueue(q: Track[]) { playbackQueue = q; }
export function getCurrentQueueIndex() { return currentQueueIndex; }
export function setCurrentQueueIndex(i: number) { currentQueueIndex = i; }
export function getSearchQuery() { return searchQuery; }
export function setSearchQuery(q: string) { searchQuery = q; }
export function getCurrentTrack() { return currentTrack; }
export function setCurrentTrack(t: Track | null) { currentTrack = t; }

export async function ensureTracksLoaded(): Promise<Track[]> {
  if (tracks.length === 0) {
    try {
      tracks = await invoke("get_tracks");
    } catch (e) {
      console.error("Failed to load tracks", e);
    }
  }
  return tracks;
}
