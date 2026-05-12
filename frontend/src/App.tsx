import { useEffect, useState } from 'react'
import { BrowserRouter as Router, Routes, Route, Navigate } from 'react-router-dom'
import { api, API_BASE, IS_TAURI } from './api/adapter'
import { listen } from '@tauri-apps/api/event'
import { Star, Wand2 } from 'lucide-react'
import toast from 'react-hot-toast'

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

export interface TaskUpdate {
  taskId: string;
  status: string;
  progress: number;
  total: number;
  message: string;
  startedAt?: number;
  finishedAt?: number;
  debugInfo?: string;
}

function App() {
  const {
    libraries,
    movies,
    tvShows,
    loading,
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

  const [tasks, setTasks] = useState<Record<string, TaskUpdate>>({})
  const [isScrolled, setIsScrolled] = useState(false);
  const [searchQuery, setSearchQuery] = useState('');
  
  const [selectedItem, setSelectedItem] = useState<any | null>(null);
  const [selectedIds, setSelectedIds] = useState<number[]>([]);
  const [selectionMode, setSelectionMode] = useState(false);
  const [showFilterMenu, setShowFilterMenu] = useState(false);
  const [appSettings, setAppSettings] = useState<Record<string, string>>({});

  useEffect(() => {
    loadSettings()
  }, [])

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

    const handleScroll = () => setIsScrolled(window.scrollY > 0);
    window.addEventListener('scroll', handleScroll);

    return () => {
      cleanupTasks();
      window.removeEventListener('scroll', handleScroll);
    };
  }, [])


  const handleTaskUpdate = (update: TaskUpdate) => {
    console.log('Frontend received task update:', update);
    setTasks(prev => {
      const oldStatus = prev[update.taskId]?.status;

      if (update.status === 'error' && oldStatus !== 'error') {
        toast.error(`Task Failed: ${update.message}`, { duration: 5000 });
      } else if (update.status === 'completed' && oldStatus !== 'completed') {
        toast.success(`Task Completed: ${update.message}`, { duration: 5000 });
      }

      return { ...prev, [update.taskId]: update };
    });
    if (update.status === 'completed') {
      setRefreshingIds({}); 
      setTimeout(loadData, 1000);
    }
  };

  const loadSettings = async () => {
    try {
      const data = await api.getSettings();
      setAppSettings(data);
    } catch (err) {
      console.error('Failed to load settings', err);
    }
  };

  const subscribeToTasks = () => {
    // Fetch initial history
    api.getTasks().then(initialTasks => {
      const taskMap: Record<string, TaskUpdate> = {};
      initialTasks.forEach((t: any) => {
        const id = t.taskId || t.task_id;
        if (id) taskMap[id] = { ...t, taskId: id };
      });
      setTasks(prev => ({ ...taskMap, ...prev }));
    }).catch(err => console.error('Failed to fetch initial tasks:', err));

    const eventSource = new EventSource(`${API_BASE}/tasks/stream`);
    eventSource.onmessage = (event) => {
      handleTaskUpdate(JSON.parse(event.data));
    };
    return () => eventSource.close();
  }

  const handleDownload = async (id: number, type: 'movie' | 'tv') => {
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
          <Route path="/tasks" element={<TasksPage tasks={Object.values(tasks)} />} />
          <Route path="/settings" element={<SettingsPage />} />
        </Routes>

        {selectedItem && (
          <DetailModal 
            item={selectedItem}
            onClose={() => setSelectedItem(null)}
            onPlay={() => {
              if (selectedItem.episodes) api.playEpisode(selectedItem.id);
              else api.playMovie(selectedItem.id);
            }}
            onRefresh={() => handleRefreshMetadata(selectedItem)}
            onProcessAdvanced={() => handleProcessAdvanced(selectedItem)}
            isRefreshing={refreshingIds[selectedItem.id]}
            onDownload={() => handleDownload(selectedItem.id, selectedItem.episodes ? 'tv' : 'movie')}
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
    </Router>
  )
}

export default App
