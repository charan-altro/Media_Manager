import { useEffect, useState } from 'react'
import { BrowserRouter as Router, Routes, Route, Navigate } from 'react-router-dom'
import { api, API_BASE, IS_TAURI } from './api/adapter'
import { listen } from '@tauri-apps/api/event'
import { Star, Wand2 } from 'lucide-react'
import toast from 'react-hot-toast'

// Components
import Navbar from './components/Navbar'
import DetailModal from './components/DetailModal'

// Pages
import MoviesPage from './pages/MoviesPage'
import TvShowsPage from './pages/TvShowsPage'
import TasksPage from './pages/TasksPage'
import SettingsPage from './pages/SettingsPage'

export interface TaskUpdate {
  task_id: string;
  status: string;
  progress: number;
  total: number;
  message: string;
  started_at?: number;
  debug_info?: string;
}

function App() {
  const [libraries, setLibraries] = useState<any[]>([])
  const [movies, setMovies] = useState<any[]>([])
  const [tvShows, setTvShows] = useState<any[]>([])
  const [tasks, setTasks] = useState<Record<string, TaskUpdate>>({})
  const [loading, setLoading] = useState(true)
  const [selectedLibrary, setSelectedLibrary] = useState<number | null>(null)
  const [isScrolled, setIsScrolled] = useState(false);
  const [searchQuery, setSearchQuery] = useState('');
  
  const [selectedItem, setSelectedItem] = useState<any | null>(null);
  const [selectedIds, setSelectedIds] = useState<number[]>([]);
  const [selectionMode, setSelectionMode] = useState(false);
  const [genreFilter, setGenreFilter] = useState('');
  const [languageFilter, setLanguageFilter] = useState('');
  const [allGenres, setAllGenres] = useState<string[]>([]);
  const [allLanguages, setAllLanguages] = useState<string[]>([]);
  const [showFilterMenu, setShowFilterMenu] = useState(false);
  const [refreshingIds, setRefreshingIds] = useState<Record<number, boolean>>({});
  const [appSettings, setAppSettings] = useState<Record<string, string>>({});
  const [currentTime, setCurrentTime] = useState(Date.now());

  useEffect(() => {
    loadData()
    loadSettings()
  }, [selectedLibrary, genreFilter, languageFilter])

  useEffect(() => {
    let cleanupTasks: () => void = () => {};

    if (IS_TAURI) {
      const unlistenPromise = listen<TaskUpdate>('task-update', (event) => {
        handleTaskUpdate(event.payload);
      });
      cleanupTasks = () => { unlistenPromise.then(unlisten => unlisten()); };
    } else {
      cleanupTasks = subscribeToTasks();
    }

    const timer = setInterval(() => setCurrentTime(Date.now()), 1000);
    const handleScroll = () => setIsScrolled(window.scrollY > 0);
    window.addEventListener('scroll', handleScroll);

    return () => {
      cleanupTasks();
      clearInterval(timer);
      window.removeEventListener('scroll', handleScroll);
    };
  }, [])


  const handleTaskUpdate = (update: TaskUpdate) => {
    setTasks(prev => {
      const oldStatus = prev[update.task_id]?.status;

      if (update.status === 'error' && oldStatus !== 'error') {
        toast.error(`Task Failed: ${update.message}`, { duration: 5000 });
      } else if (update.status === 'completed' && oldStatus !== 'completed') {
        toast.success(`Task Completed: ${update.message}`, { duration: 5000 });
      }

      return { ...prev, [update.task_id]: update };
    });
    if (update.status === 'completed') {
      setRefreshingIds({}); 
      setTimeout(loadData, 1000);
    }
  };

  const loadData = async () => {
    try {
      const [libs, movs, shows, genres, langs] = await Promise.all([
        api.getLibraries(),
        api.getMovies(selectedLibrary || undefined, genreFilter, languageFilter),
        api.getTvShows(selectedLibrary || undefined, genreFilter, languageFilter),
        api.getGenres(),
        api.getLanguages()
      ]);
      
      setLibraries(libs)
      setMovies(movs)
      setTvShows(shows)
      setAllGenres(genres)
      setAllLanguages(langs)
    } catch (err) {
      console.error('Failed to load data', err)
    } finally {
      setLoading(false)
    }
  }

  const loadSettings = async () => {
    try {
      const data = await api.getSettings();
      setAppSettings(data);
    } catch (err) {
      console.error('Failed to load settings', err);
    }
  };

  const subscribeToTasks = () => {
    const eventSource = new EventSource(`${API_BASE}/tasks/stream`);
    eventSource.onmessage = (event) => {
      handleTaskUpdate(JSON.parse(event.data));
    };
    return () => eventSource.close();
  }

  const handleRefreshMetadata = async (id: number) => {
    if (refreshingIds[id]) return;
    setRefreshingIds(prev => ({ ...prev, [id]: true }));
    try {
      await api.refreshMetadata(id);
    } catch (err) {
      console.error('Failed to refresh metadata', err);
      setRefreshingIds(prev => ({ ...prev, [id]: false }));
    }
  }

  const handleProcessAdvanced = async (id: number) => {
    if (refreshingIds[id]) return;
    setRefreshingIds(prev => ({ ...prev, [id]: true }));
    try {
      await api.request('process_movie_advanced', `/movies/${id}/process-advanced`, { method: 'POST' });
      toast.success('Advanced analysis started in background.');
    } catch (err) {
      console.error('Failed to start advanced analysis', err);
      setRefreshingIds(prev => ({ ...prev, [id]: false }));
    }
  };

  const handleDownload = async (id: number, type: 'movie' | 'tv') => {
    if (IS_TAURI) {
      try {
        const dest = window.prompt("Enter destination directory path:");
        if (!dest) return;
        await api.downloadToLocal(id, type, dest + '\\downloaded_' + id);
        toast.success('Download complete!');
      } catch (err: any) {
        toast.error('Download failed: ' + err.message);
      }
    } else {
      window.open(`${API_BASE}/${type === 'movie' ? 'movies' : 'episodes'}/${id}/download`);
    }
  }

  const runningTasks = Object.values(tasks).filter(t => t.status === 'running');
  const latestTask = runningTasks[runningTasks.length - 1];

  return (
    <Router>
      <div className="min-h-screen bg-zinc-950 font-sans selection:bg-red-600/30 selection:text-red-500">
        <Navbar 
          isScrolled={isScrolled}
          searchQuery={searchQuery}
          setSearchQuery={setSearchQuery}
          hasRunningTasks={runningTasks.length > 0}
          latestTask={latestTask}
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
              onItemClick={(item) => setSelectedItem(item)}
              onPlayClick={(item, e) => { e.stopPropagation(); api.playMovie(item.id); }}
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
              onItemClick={(item) => setSelectedItem(item)}
              onPlayClick={(item, e) => { e.stopPropagation(); setSelectedItem(item); }}
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
          <Route path="/tasks" element={<TasksPage tasks={Object.values(tasks)} currentTime={currentTime} />} />
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
          <Route path="*" element={<Navigate to="/" />} />
        </Routes>

        {loading && movies.length === 0 && tvShows.length === 0 && (
          <div className="flex flex-col items-center justify-center py-40 gap-6">
            <svg className="animate-spin h-12 w-12 text-red-600" xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24">
              <circle className="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" strokeWidth="4"></circle>
              <path className="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z"></path>
            </svg>
            <p className="text-zinc-500 font-black uppercase tracking-widest text-xs animate-pulse">Initializing Media Library...</p>
          </div>
        )}

        {selectedItem && (
          <DetailModal 
            item={selectedItem}
            onClose={() => setSelectedItem(null)}
            onRefresh={handleRefreshMetadata}
            onAdvanced={handleProcessAdvanced}
            onDownload={handleDownload}
            refreshingIds={refreshingIds}
            loadData={loadData}
          />
        )}

        {selectionMode && selectedIds.length > 0 && (
          <div className="fixed bottom-8 left-1/2 -translate-x-1/2 bg-zinc-900/90 backdrop-blur-xl border border-red-500/20 rounded-2xl px-8 py-5 shadow-[0_20px_50px_rgba(0,0,0,0.5)] flex items-center gap-8 z-50 animate-in slide-in-from-bottom-10">
            <div className="flex flex-col">
              <span className="text-white font-black uppercase italic tracking-tighter text-lg">{selectedIds.length}</span>
              <span className="text-[10px] font-black uppercase tracking-[0.2em] text-zinc-500">Selected</span>
            </div>
            <div className="w-px h-10 bg-zinc-800" />
            <div className="flex items-center gap-6">
              <button 
                onClick={() => {
                  api.scrapeBatch(selectedIds, window.location.pathname === '/tv' ? 'tv' : 'movie');
                  setSelectionMode(false);
                  setSelectedIds([]);
                }}
                className="flex items-center gap-2 text-xs font-black uppercase tracking-widest text-red-500 hover:text-red-400 transition group"
              >
                <Star className="w-4 h-4" /> Scrape Matches
              </button>
              <button
                onClick={() => {
                  api.cleanupBatch(selectedIds, window.location.pathname === '/tv' ? 'tv' : 'movie');
                  setSelectionMode(false);
                  setSelectedIds([]);
                }}
                className="flex items-center gap-2 text-xs font-black uppercase tracking-widest text-zinc-400 hover:text-white transition group"
              >
                <Wand2 className="w-4 h-4 text-red-500" /> Deep Cleanup & Rename
              </button>
              <div className="w-px h-6 bg-zinc-800" />
              <button 
                onClick={() => {
                  setSelectionMode(false);
                  setSelectedIds([]);
                }}
                className="text-xs font-black uppercase tracking-widest text-zinc-600 hover:text-zinc-400 transition"
              >
                Cancel
              </button>
            </div>
          </div>
        )}
      </div>
    </Router>
  )
}

export default App
