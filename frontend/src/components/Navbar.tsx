import React from 'react';
import { Film, Bell, User, Search, Activity } from 'lucide-react';
import { Link, useLocation } from 'react-router-dom';
import { useTasks } from '../context/TaskContext';

interface NavbarProps {
  isScrolled: boolean;
  searchQuery: string;
  setSearchQuery: (query: string) => void;
}

const Navbar: React.FC<NavbarProps> = ({ 
  isScrolled, 
  searchQuery, 
  setSearchQuery, 
}) => {
  const { runningTasks, latestTask } = useTasks();
  const hasRunningTasks = runningTasks.length > 0;
  const location = useLocation();
  const currentPath = location.pathname;

  return (
    <>
      <nav className={`fixed top-0 w-full z-50 transition-all duration-500 px-4 md:px-12 py-4 flex items-center justify-between ${isScrolled ? 'bg-zinc-950 shadow-2xl' : 'bg-gradient-to-b from-black/80 to-transparent'}`}>
        <div className="flex items-center gap-10">
          <Link to="/" className="flex items-center gap-2">
            <div className="w-8 h-8 bg-red-600 rounded flex items-center justify-center shadow-lg shadow-red-900/20">
              <Film className="w-5 h-5 text-white" />
            </div>
            <h1 className="text-xl font-black tracking-tighter text-white uppercase italic">Media<span className="text-red-600">Vault</span></h1>
          </Link>
          
          <div className="hidden md:flex items-center gap-6">
            <Link to="/" className={`text-sm font-medium transition ${currentPath === '/' ? 'text-white' : 'text-zinc-400 hover:text-zinc-200'}`}>Movies</Link>
            <Link to="/tv" className={`text-sm font-medium transition ${currentPath === '/tv' ? 'text-white' : 'text-zinc-400 hover:text-zinc-200'}`}>TV Shows</Link>
            <Link to="/tasks" className={`text-sm font-medium transition flex items-center gap-2 ${currentPath === '/tasks' ? 'text-white' : 'text-zinc-400 hover:text-zinc-200'}`}>
              Activity {hasRunningTasks && <span className="flex h-2 w-2 rounded-full bg-red-500 animate-pulse" />}
            </Link>
            <Link to="/settings" className={`text-sm font-medium transition ${currentPath === '/settings' ? 'text-white' : 'text-zinc-400 hover:text-zinc-200'}`}>Settings</Link>
          </div>
        </div>

        <div className="flex items-center gap-6">
          <div className="relative group hidden sm:block">
            <Search className="w-5 h-5 text-zinc-400 absolute left-3 top-1/2 -translate-y-1/2 group-focus-within:text-red-500 transition" />
            <input 
              type="text" 
              placeholder="Titles, genres, actors..." 
              value={searchQuery}
              onChange={(e) => setSearchQuery(e.target.value)}
              className="bg-black/40 border border-zinc-800 rounded-full py-1.5 pl-10 pr-4 text-sm text-white focus:outline-none focus:ring-1 focus:ring-red-600 focus:border-red-600 w-48 md:w-64 transition-all"
            />
          </div>
          <Bell className="w-5 h-5 text-zinc-400 hover:text-white cursor-pointer transition" />
          <div className="w-8 h-8 bg-zinc-800 rounded cursor-pointer flex items-center justify-center border border-zinc-700 hover:border-zinc-500 transition">
            <User className="w-4 h-4 text-zinc-400" />
          </div>
        </div>
      </nav>
      
      {/* Global Progress Bar */}
      {hasRunningTasks && latestTask && (
        <div className="fixed top-16 left-0 w-full z-40 bg-zinc-900/80 backdrop-blur-md border-b border-zinc-800 animate-in slide-in-from-top-2">
          <div className="px-4 md:px-12 py-2 flex items-center gap-4">
            <Activity className="w-4 h-4 text-red-500 animate-pulse" />
            <div className="flex-1">
              <div className="flex justify-between items-center mb-1">
                <span className="text-[10px] font-bold text-zinc-300 uppercase tracking-widest">{latestTask.message}</span>
                <span className="text-[10px] font-mono text-zinc-500">{latestTask.total > 0 ? Math.round((latestTask.progress / latestTask.total) * 100) : 0}%</span>
              </div>
              <div className="h-1 w-full bg-zinc-800 rounded-full overflow-hidden">
                <div 
                  className="h-full bg-red-600 transition-all duration-300"
                  style={{ width: `${latestTask.total > 0 ? (latestTask.progress / latestTask.total) * 100 : 0}%` }}
                />
              </div>
            </div>
          </div>
        </div>
      )}
    </>
  );
};

export default Navbar;
