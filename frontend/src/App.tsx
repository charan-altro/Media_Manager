import { useState, useEffect, useCallback } from 'react'
import { BrowserRouter as Router, Routes, Route } from 'react-router-dom'
import { api, IS_TAURI, API_BASE } from './api/adapter'
import { Star, Wand2 } from 'lucide-react'
import toast from 'react-hot-toast'
import { TaskProvider } from './context/TaskContext'
import { MediaStoreProvider, useMediaStore } from './context/MediaStoreContext'

// Components
import Navbar from './components/Navbar'
import DetailModal from './components/DetailModal'

// Pages
import MoviesPage from './pages/MoviesPage'
import TvShowsPage from './pages/TvShowsPage'
import TasksPage from './pages/TasksPage'
import SettingsPage from './pages/SettingsPage'

// ─── Inner app (consumes context) ─────────────────────────────────────────────

function AppContent() {
  const {
    libraries,
    selectedLibrary,
    setSelectedLibrary,
    searchQuery,
    setSearchQuery,
    selectedItem,
    setSelectedItem,
    selectedIds,
    setSelectedIds,
    selectionMode,
    setSelectionMode,
    refreshingIds,
    setRefreshingIds,
    handleRefreshMetadata,
    handleProcessAdvanced,
    appSettings,
    setAppSettings,
    loadData,
  } = useMediaStore();

  const loadSettings = useCallback(async () => {
    try {
      const data = await api.getSettings();
      setAppSettings(data);
    } catch (err) {
      console.error('Failed to load settings', err);
    }
  }, [setAppSettings]);

  const [isScrolled, setIsScrolled] = useState(false);

  useEffect(() => {
    const handleScroll = () => {
      setIsScrolled(window.scrollY > 30);
    };
    window.addEventListener('scroll', handleScroll, { passive: true });
    return () => window.removeEventListener('scroll', handleScroll);
  }, []);

  useEffect(() => { loadSettings(); }, [loadSettings]);

  const handleDownload = useCallback(async (id: number, type: 'movie' | 'tv') => {
    if (IS_TAURI) {
      try {
        const dest = window.prompt('Enter destination directory path:');
        if (!dest) return;
        const result = await api.downloadToLocal(id, type, dest);
        toast.success(result);
      } catch (err: unknown) {
        const message = err instanceof Error ? err.message : String(err);
        toast.error('Download failed: ' + message);
      }
    } else {
      window.open(`${API_BASE}/${type === 'movie' ? 'movies' : 'episodes'}/${id}/download`);
    }
  }, []);

  return (
    <TaskProvider loadData={loadData} setRefreshingIds={setRefreshingIds}>
    <div className="min-h-screen bg-zinc-950 font-sans selection:bg-red-600/30 selection:text-red-500">
      <Navbar
        isScrolled={isScrolled}
        searchQuery={searchQuery}
        setSearchQuery={setSearchQuery}
      />

      <Routes>
        <Route path="/" element={<MoviesPage />} />
        <Route path="/tv" element={<TvShowsPage />} />
        <Route path="/tasks" element={<TasksPage />} />
        <Route
          path="/settings"
          element={
            <SettingsPage
              appSettings={appSettings}
              setAppSettings={setAppSettings}
              libraries={libraries}
              selectedLibrary={selectedLibrary}
              setSelectedLibrary={setSelectedLibrary}
              loadData={loadData}
            />
          }
        />
      </Routes>

      {selectedItem && (
        <DetailModal
          item={selectedItem}
          onClose={() => setSelectedItem(null)}
          onRefresh={() => handleRefreshMetadata(selectedItem.id)}
          onAdvanced={(id, type) => handleProcessAdvanced(id, type)}
          onDownload={handleDownload}
          refreshingIds={refreshingIds}
          loadData={loadData}
        />
      )}

      {selectionMode && selectedIds.length > 0 && (
        <div className="fixed bottom-8 left-1/2 -translate-x-1/2 z-50 bg-zinc-900/90 backdrop-blur-md border border-zinc-800 px-6 py-4 rounded-2xl shadow-2xl flex items-center gap-6 animate-in fade-in slide-in-from-bottom-4 duration-300">
          <div className="flex items-center gap-3 border-r border-zinc-800 pr-6">
            <span className="h-2 w-2 rounded-full bg-red-500 animate-pulse" />
            <span className="text-zinc-100 font-medium">{selectedIds.length} items selected</span>
          </div>

          <div className="flex items-center gap-3">
            <button
              onClick={() => {
                const mType = window.location.pathname === '/tv' ? 'tv' : 'movie';
                api.scrapeBatch(selectedIds, mType);
                setSelectionMode(false);
                setSelectedIds([]);
              }}
              className="flex items-center gap-2 bg-zinc-100 hover:bg-white text-zinc-950 px-4 py-2 rounded-xl text-sm font-semibold transition-all active:scale-95"
            >
              <Star className="w-4 h-4 fill-zinc-950" />
              Enrich Data
            </button>

            <button
              onClick={() => {
                const mType = window.location.pathname === '/tv' ? 'tv' : 'movie';
                api.cleanupBatch(selectedIds, mType);
                setSelectionMode(false);
                setSelectedIds([]);
              }}
              className="flex items-center gap-2 bg-zinc-800 hover:bg-zinc-700 text-zinc-100 px-4 py-2 rounded-xl text-sm font-semibold transition-all active:scale-95"
            >
              <Wand2 className="w-4 h-4" />
              Cleanup Folders
            </button>

            <button
              onClick={() => { setSelectionMode(false); setSelectedIds([]); }}
              className="ml-2 text-xs font-bold uppercase tracking-widest text-zinc-600 hover:text-zinc-400 transition"
            >
              Cancel
            </button>
          </div>
        </div>
      )}
    </div>
    </TaskProvider>
  );
}

// ─── Root (provides context) ──────────────────────────────────────────────────

function App() {
  return (
    <Router>
      <MediaStoreProvider>
        <AppContent />
      </MediaStoreProvider>
    </Router>
  );
}

export default App
