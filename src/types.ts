export interface Track {
  id: number;
  path: string;
  title: string | null;
  artist: string | null;
  album: string | null;
  genre: string | null;
  year: number | null;
  cover_art: string | null;
  duration: number | null;
  last_modified: number | null;
}

export interface Album {
  name: string;
  artist: string;
  tracks: Track[];
  cover: string | null;
}

export interface LyricLine {
  time: number;
  text: string;
}

export type ViewMode = 'tracks' | 'albums' | 'album-details' | 'sync';
