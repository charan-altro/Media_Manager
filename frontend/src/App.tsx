import { useEffect, useState, useCallback } from 'react'
import { BrowserRouter as Router, Routes, Route } from 'react-router-dom'
import { api, IS_TAURI, API_BASE } from './api/adapter'
import { Star, Wand2 } from 'lucide-react'
import toast from 'react-hot-toast'
import { TaskProvider } from './context/TaskContext'

// Hooks
import { useLibraryData } from './hooks/useLibraryData'
import { useMediaActions } from './hooks/useMediaActions'

// Components
import Navbar from './components/Navbar'
import DetailModal from './components/DetailModal'

// Pages
import MoviesPage from './pages/MoviesPage'
import TvShowsPage from './pages/TvShowsPage'
import TasksPage from './pages/TasksPage'
import SettingsPage from './pages/SettingsPage'

function App() {
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
    loadData
  } = useLibraryData();

  const {
    refreshingIds,
    setRefreshingIds,
    handleRefreshMetadata,
    handleProcessAdvanced
  } = useMediaActions();

  const [isScrolled, setIsScrolled] = useState(false);
  const [searchQuery, setSearchQuery] = useState('');
  
  const [selectedItem, setSelectedItem] = useState<any | null>(null);
  const [selectedIds, setSelectedIds] = useState<number[]>([]);
  const [selectionMode, setSelectionMode] = useState(false);
  const [showFilterMenu, setShowFilterMenu] = useState(false);
  const [appSettings, setAppSettings] = useState<Record<string, string>>({});

  const loadSettings = useCallback(async () => {
    try {
      const data = await api.getSettings();
      setAppSettings(data);
    } catch (err) {
      console.error('Failed to load settings', err);
    }
  }, []);

  useEffect(() => {
    loadSettings()
  }, [loadSettings])

  useEffect(() => {
    const handleScroll = () => setIsScrolled(window.scrollY > 0);
    window.addEventListener('scroll', handleScroll);

    return () => {
      window.removeEventListener('scroll', handleScroll);
    };
  }, [])

  const handleDownload = useCallback(async (id: number, type: 'movie' | 'tv') => {
    if (IS_TAURI) {
      try {
        const dest = window.prompt("Enter destination directory path:");
        if (!dest) return;
        const result = await api.downloadToLocal(id, type, dest);
        toast.success(result);
      } catch (err: any) {
        toast.error('Download failed: ' + err.message);
      }
    } else {
      window.open(`${API_BASE}/${type === 'movie' ? 'movies' : 'episodes'}/${id}/download`);
    }
  }, []);

  const handleItemClick = useCallback((item: any) => setSelectedItem(item), []);
  const handlePlayClick = useCallback((item: any, e: React.MouseEvent) => { 
    e.stopPropagation(); 
    setSelectedItem(item); 
  }, []);

  return (
    <Router>
      <TaskProvider loadData={loadData} setRefreshingIds={setRefreshingIds}>
        <div className="min-h-screen bg-zinc-950 font-sans selection:bg-red-600/30 selection:text-red-500">
          <Navbar 
            isScrolled={isScrolled}
            searchQuery={searchQuery}
            setSearchQuery={setSearchQuery}
          />
          
          <Routes>
            <Route path="/" element={
              <MoviesPage 
                movies={movies}
                libraries={libraries}
                selectedLibrary={selectedLibrary}
                setSelectedLibrary={setSelectedLibrary}
                tvShowsCount={tvShows.length}
                searchQuery={searchQuery}
                onItemClick={handleItemClick}
                onPlayClick={handlePlayClick}
                selectedIds={selectedIds}
                selectionMode={selectionMode}
                setSelectionMode={setSelectionMode}
                setSelectedIds={setSelectedIds}
                genreFilter={genreFilter}
                setGenreFilter={setGenreFilter}
                languageFilter={languageFilter}
                setLanguageFilter={setLanguageFilter}
                allGenres={allGenres}
                allLanguages={allLanguages}
                showFilterMenu={showFilterMenu}
                setShowFilterMenu={setShowFilterMenu}
              />
            } />
            <Route path="/tv" element={
              <TvShowsPage 
                tvShows={tvShows}
                libraries={libraries}
                selectedLibrary={selectedLibrary}
                setSelectedLibrary={setSelectedLibrary}
                moviesCount={movies.length}
                searchQuery={searchQuery}
                onItemClick={handleItemClick}
                onPlayClick={handlePlayClick}
                selectedIds={selectedIds}
                selectionMode={selectionMode}
                setSelectionMode={setSelectionMode}
                setSelectedIds={setSelectedIds}
                genreFilter={genreFilter}
                setGenreFilter={setGenreFilter}
                languageFilter={languageFilter}
                setLanguageFilter={setLanguageFilter}
                allGenres={allGenres}
                allLanguages={allLanguages}
                showFilterMenu={showFilterMenu}
                setShowFilterMenu={setShowFilterMenu}
              />
            } />
            <Route path="/tasks" element={<TasksPage />} />
            <Route path="/settings" element={
              <SettingsPage 
                appSettings={appSettings}
                setAppSettings={setAppSettings}
                libraries={libraries}
                selectedLibrary={selectedLibrary}
                setSelectedLibrary={setSelectedLibrary}
                loadData={loadData}
              />
            } />
          </Routes>

          {selectedItem && (
            <DetailModal 
              item={selectedItem}
              onClose={() => setSelectedItem(null)}
              onRefresh={() => handleRefreshMetadata(selectedItem.id)}
              onAdvanced={() => handleProcessAdvanced(selectedItem.id)}
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
                  onClick={() => {
                    setSelectionMode(false);
                    setSelectedIds([]);
                  }}
                  className="ml-2 text-xs font-bold uppercase tracking-widest text-zinc-600 hover:text-zinc-400 transition"
                >
                  Cancel
                </button>
              </div>
            </div>
          )}
        </div>
      </TaskProvider>
    </Router>
  )
}

export default App
