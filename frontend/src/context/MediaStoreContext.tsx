// frontend/src/context/MediaStoreContext.tsx
import {
  createContext,
  useContext,
  useState,
  useCallback,
  type ReactNode,
} from 'react';
import type { Movie, TVShow, Library } from '../api/adapter';
import { useLibraryData } from '../hooks/useLibraryData';
import { useMediaActions } from '../hooks/useMediaActions';

// ─── Types ────────────────────────────────────────────────────────────────────

export interface MediaStoreState {
  // Library data
  libraries: Library[];
  movies: Movie[];
  tvShows: TVShow[];
  selectedLibrary: number | null;
  setSelectedLibrary: (id: number | null) => void;
  loadData: () => Promise<void>;

  // Filters
  searchQuery: string;
  setSearchQuery: (q: string) => void;
  genreFilter: string;
  setGenreFilter: (genre: string) => void;
  languageFilter: string;
  setLanguageFilter: (lang: string) => void;
  allGenres: string[];
  allLanguages: string[];
  showFilterMenu: boolean;
  setShowFilterMenu: (show: boolean) => void;

  // Selection
  selectedIds: number[];
  setSelectedIds: (ids: number[] | ((prev: number[]) => number[])) => void;
  selectionMode: boolean;
  setSelectionMode: (mode: boolean) => void;

  // Detail modal
  selectedItem: Movie | TVShow | null;
  setSelectedItem: (item: Movie | TVShow | null) => void;
  handleItemClick: (item: Movie | TVShow) => void;
  handlePlayClick: (item: Movie | TVShow, e: React.MouseEvent) => void;

  // Actions
  refreshingIds: Record<number, boolean>;
  setRefreshingIds: (ids: Record<number, boolean> | ((prev: Record<number, boolean>) => Record<number, boolean>)) => void;
  handleRefreshMetadata: (id: number) => Promise<void>;
  handleProcessAdvanced: (id: number) => Promise<void>;

  // Settings
  appSettings: Record<string, string>;
  setAppSettings: (s: Record<string, string>) => void;
}

// ─── Context ─────────────────────────────────────────────────────────────────

const MediaStoreContext = createContext<MediaStoreState | null>(null);

// ─── Provider ─────────────────────────────────────────────────────────────────

interface MediaStoreProviderProps {
  children: ReactNode;
}

export function MediaStoreProvider({ children }: MediaStoreProviderProps) {
  const {
    libraries,
    movies,
    tvShows,
    selectedLibrary,
    setSelectedLibrary,
    genreFilter,
    setGenreFilter,
    languageFilter,
    setLanguageFilter,
    allGenres,
    allLanguages,
    loadData,
  } = useLibraryData();

  const { refreshingIds, setRefreshingIds, handleRefreshMetadata, handleProcessAdvanced } =
    useMediaActions();

  // UI State
  const [searchQuery, setSearchQuery] = useState('');
  const [showFilterMenu, setShowFilterMenu] = useState(false);
  const [selectedIds, setSelectedIds] = useState<number[]>([]);
  const [selectionMode, setSelectionMode] = useState(false);
  const [selectedItem, setSelectedItem] = useState<Movie | TVShow | null>(null);
  const [appSettings, setAppSettings] = useState<Record<string, string>>({});

  const handleItemClick = useCallback((item: Movie | TVShow) => setSelectedItem(item), []);
  const handlePlayClick = useCallback((item: Movie | TVShow, e: React.MouseEvent) => {
    e.stopPropagation();
    setSelectedItem(item);
  }, []);

  const value: MediaStoreState = {
    // Library data
    libraries,
    movies,
    tvShows,
    selectedLibrary,
    setSelectedLibrary,
    loadData,

    // Filters
    searchQuery,
    setSearchQuery,
    genreFilter,
    setGenreFilter,
    languageFilter,
    setLanguageFilter,
    allGenres,
    allLanguages,
    showFilterMenu,
    setShowFilterMenu,

    // Selection
    selectedIds,
    setSelectedIds,
    selectionMode,
    setSelectionMode,

    // Detail modal
    selectedItem,
    setSelectedItem,
    handleItemClick,
    handlePlayClick,

    // Actions
    refreshingIds,
    setRefreshingIds,
    handleRefreshMetadata,
    handleProcessAdvanced,

    // Settings
    appSettings,
    setAppSettings,
  };

  return (
    <MediaStoreContext.Provider value={value}>
      {children}
    </MediaStoreContext.Provider>
  );
}

// ─── Hook ─────────────────────────────────────────────────────────────────────

export function useMediaStore(): MediaStoreState {
  const ctx = useContext(MediaStoreContext);
  if (!ctx) {
    throw new Error('useMediaStore must be used within a MediaStoreProvider');
  }
  return ctx;
}
