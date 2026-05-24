import React from 'react';
import { Film, Tv, Trash2 } from 'lucide-react';
import { api } from '../api/adapter';
import toast from 'react-hot-toast';

interface SettingsPageProps {
  appSettings: Record<string, string>;
  setAppSettings: (settings: Record<string, string>) => void;
  libraries: any[];
  selectedLibrary: number | null;
  setSelectedLibrary: (id: number | null) => void;
  loadData: () => void;
}

const SettingsPage: React.FC<SettingsPageProps> = ({ 
  appSettings, setAppSettings, libraries, selectedLibrary, setSelectedLibrary, loadData 
}) => {
  const [isAdding, setIsAdding] = React.useState(false);

  return (
    <div className="px-4 md:px-12 py-24 min-h-screen">
      <div className="max-w-4xl mx-auto space-y-12">
        <div className="space-y-2">
          <h3 className="text-4xl font-black text-white uppercase italic tracking-tighter">Settings</h3>
          <p className="text-zinc-500 font-medium">Configure your media sources and system preferences.</p>
        </div>

        <div className="bg-[#181818] p-8 md:p-10 rounded-2xl border border-zinc-800 shadow-2xl space-y-8">
          <div className="space-y-2">
            <h4 className="text-xl font-black text-white uppercase italic">Scraper API Keys</h4>
            <p className="text-zinc-500 text-sm">Enter your API keys for various metadata providers.</p>
          </div>
          <div className="grid gap-6">
            <div className="grid md:grid-cols-2 gap-6">
              {[
                { label: 'TMDB API Key', key: 'tmdb_api_key' },
                { label: 'OMDB API Key', key: 'omdb_api_key' },
                { label: 'TVDB API Key', key: 'tvdb_api_key' },
                { label: 'Fanart.tv API Key', key: 'fanart_api_key' },
                { label: 'Trakt.tv Client ID', key: 'trakt_api_key' },
                { label: 'Trakt.tv Access Token', key: 'trakt_access_token' },
                { label: 'AniDB Client Name', key: 'anidb_client', type: 'text' },
                { label: 'TVMaze API Key', key: 'tvmaze_api_key' },
                { label: 'IMDbAPI.dev Key', key: 'imdbapi_key' },
                { label: 'MPDb Abo Key', key: 'mpdb_abo_key' },
                { label: 'MPDb Username', key: 'mpdb_username', type: 'text' },
              ].map(field => (
                <div key={field.key} className="space-y-2">
                  <label className="text-xs font-black text-zinc-500 uppercase tracking-[0.2em]">{field.label}</label>
                  <input 
                    type={field.type || "password"}
                    value={appSettings[field.key] || ''}
                    onChange={(e) => setAppSettings({...appSettings, [field.key]: e.target.value})}
                    className="w-full bg-black border border-zinc-800 rounded-xl p-4 text-sm font-medium text-white focus:border-red-600 outline-none transition shadow-inner" 
                  />
                </div>
              ))}
              <div className="space-y-2 md:col-span-2">
                <label className="text-xs font-black text-zinc-500 uppercase tracking-[0.2em]">Discord Webhook URL</label>
                <input 
                  placeholder="https://discord.com/api/webhooks/..."
                  value={appSettings.discord_webhook_url || ''}
                  onChange={(e) => setAppSettings({...appSettings, discord_webhook_url: e.target.value})}
                  className="w-full bg-black border border-zinc-800 rounded-xl p-4 text-sm font-medium text-white focus:border-red-600 outline-none transition shadow-inner" 
                />
              </div>
            </div>
            <div className="flex gap-4">
              <button 
                onClick={async () => {
                  try {
                    await api.setSettings(appSettings);
                    toast.success('Settings saved successfully!');
                  } catch(err: any) {
                    toast.error('Failed to save settings: ' + err.message);
                  }
                }}
                className="bg-red-600 hover:bg-red-700 px-10 py-4 rounded-xl text-sm font-black transition uppercase tracking-widest text-white shadow-xl shadow-red-900/30 w-fit active:scale-95"
              >
                Save API Keys
              </button>
              <button 
                onClick={async () => {
                  try {
                    await api.syncTrakt();
                    toast.success('Library successfully synced with Trakt.tv!');
                  } catch(err: any) {
                    toast.error('Trakt sync failed: ' + err.message);
                  }
                }}
                className="bg-[#3e2a86] hover:bg-[#4b359c] px-10 py-4 rounded-xl text-sm font-black transition uppercase tracking-widest text-white shadow-xl shadow-[#3e2a86]/30 w-fit active:scale-95"
              >
                Sync with Trakt.tv
              </button>
            </div>
          </div>
        </div>

        <div className="bg-[#181818] p-8 md:p-10 rounded-2xl border border-zinc-800 shadow-2xl space-y-8">
          <div className="space-y-2">
            <h4 className="text-xl font-black text-white uppercase italic">Add New Source</h4>
            <p className="text-zinc-500 text-sm">Mount a local directory containing your movie or TV show files.</p>
          </div>
          <form onSubmit={async (e) => {
            e.preventDefault();
            if (isAdding) return;
            setIsAdding(true);
            const form = e.currentTarget;
            const formData = new FormData(form);
            try {
              const name = formData.get('name') as string;
              const path = formData.get('path') as string;
              const type = formData.get('type') as string;
              await api.createLibrary(name, path, type);
              setTimeout(loadData, 500); // Small delay to ensure DB write visibility
              form.reset();
              toast.success('Library added successfully! Automatic scan has started.');
            } catch(err: any) {
              toast.error('Failed to add library: ' + err.message);
            } finally {
              setIsAdding(false);
            }
          }} className="grid gap-6">
            <div className="grid md:grid-cols-2 gap-6">
              <div className="space-y-2">
                <label className="text-xs font-black text-zinc-500 uppercase tracking-[0.2em]">Source Name</label>
                <input name="name" placeholder="e.g. 4K Movies" className="w-full bg-black border border-zinc-800 rounded-xl p-4 text-sm font-medium text-white focus:border-red-600 focus:ring-1 focus:ring-red-600 outline-none transition shadow-inner" required />
              </div>
              <div className="space-y-2">
                <label className="text-xs font-black text-zinc-500 uppercase tracking-[0.2em]">Media Type</label>
                <select name="type" className="w-full bg-black border border-zinc-800 rounded-xl p-4 text-sm font-medium text-white focus:border-red-600 focus:ring-1 focus:ring-red-600 outline-none transition appearance-none shadow-inner">
                  <option value="movie">Movies</option>
                  <option value="tv">TV Shows</option>
                </select>
              </div>
            </div>
            <div className="space-y-2">
              <label className="text-xs font-black text-zinc-500 uppercase tracking-[0.2em]">Directory Path</label>
              <input name="path" placeholder="/media/movies" className="w-full bg-black border border-zinc-800 rounded-xl p-4 text-sm font-medium text-white focus:border-red-600 focus:ring-1 focus:ring-red-600 outline-none transition shadow-inner" required />

            </div>
            <button 
              type="submit" 
              disabled={isAdding}
              className={`${isAdding ? 'opacity-50 cursor-not-allowed' : 'hover:bg-red-700'} bg-red-600 px-10 py-4 rounded-xl text-sm font-black transition uppercase tracking-widest text-white shadow-xl shadow-red-900/30 w-fit active:scale-95`}
            >
              {isAdding ? 'Adding Source...' : 'Add Source'}
            </button>
          </form>
        </div>

        <div className="bg-[#181818] p-8 md:p-10 rounded-2xl border border-zinc-800 shadow-2xl space-y-8">
          <div className="space-y-2">
            <h4 className="text-xl font-black text-white uppercase italic">System Maintenance</h4>
            <p className="text-zinc-500 text-sm">Backup your data or check for system updates.</p>
          </div>
          <div className="flex flex-wrap gap-4">
            <button 
              onClick={async () => {
                try {
                  await api.createBackup();
                  toast.success(`Backup successful!\nStored in the 'backups' folder.`);
                } catch(err: any) {
                  toast.error('Backup failed: ' + err.message);
                }
              }}
              className="bg-zinc-800 hover:bg-zinc-700 px-8 py-4 rounded-xl text-xs font-black uppercase tracking-widest text-white transition border border-zinc-700 active:scale-95"
            >
              Create Database Backup
            </button>
            <button 
              onClick={async () => {
                try {
                  const data = await api.checkUpdates();
                  toast(`System Update Check:\nCurrent: 0.1.0\nLatest: ${data.latest_version}\n\nNo updates required at this time.`, { duration: 5000 });
                } catch(err: any) {
                  toast.error('Update check failed: ' + err.message);
                }
              }}
              className="bg-zinc-800 hover:bg-zinc-700 px-8 py-4 rounded-xl text-xs font-black uppercase tracking-widest text-zinc-400 transition border border-zinc-700 active:scale-95"
            >
              Check for Updates
            </button>
            <button 
              onClick={() => api.exportCsv()}
              className="bg-zinc-800 hover:bg-zinc-700 px-8 py-4 rounded-xl text-xs font-black uppercase tracking-widest text-white transition border border-zinc-700 active:scale-95"
            >
              Export to CSV
            </button>
            <button 
              onClick={() => api.exportHtml()}
              className="bg-zinc-800 hover:bg-zinc-700 px-8 py-4 rounded-xl text-xs font-black uppercase tracking-widest text-white transition border border-zinc-700 active:scale-95"
            >
              Export to HTML
            </button>
            <button 
              onClick={() => api.exportXlsx()}
              className="bg-zinc-800 hover:bg-zinc-700 px-8 py-4 rounded-xl text-xs font-black uppercase tracking-widest text-white transition border border-zinc-700 active:scale-95"
            >
              Export to XLSX
            </button>
            <button 
              onClick={() => api.exportJson()}
              className="bg-zinc-800 hover:bg-zinc-700 px-8 py-4 rounded-xl text-xs font-black uppercase tracking-widest text-white transition border border-zinc-700 active:scale-95"
            >
              Export to JSON
            </button>
          </div>
        </div>

        <div className="space-y-6">
          <h4 className="text-xs font-black uppercase tracking-[0.2em] text-zinc-500 px-2">Active Sources</h4>
          <div className="grid gap-4">
            {libraries.map(lib => (
              <div key={lib.id} className="flex flex-col sm:flex-row items-start sm:items-center justify-between bg-[#181818] p-6 rounded-2xl border border-zinc-800 group hover:border-zinc-600 transition shadow-xl gap-4">
                <div className="flex items-center gap-4">
                  <div className="w-12 h-12 bg-zinc-900 rounded-xl flex items-center justify-center border border-zinc-800 group-hover:bg-red-600/10 group-hover:border-red-600/30 transition">
                    {lib.media_type === 'movie' ? <Film className="w-6 h-6 text-zinc-500 group-hover:text-red-600 transition" /> : <Tv className="w-6 h-6 text-zinc-500 group-hover:text-red-600 transition" />}
                  </div>
                  <div>
                    <div className="font-black text-white uppercase italic tracking-tight text-lg">{lib.name}</div>
                    <div className="text-xs text-zinc-500 font-mono mt-1 break-all">{lib.path}</div>
                  </div>
                </div>
                <div className="flex items-center gap-3 w-full sm:w-auto">
                  <button 
                    onClick={async () => {
                      if (confirm(`Are you sure you want to remove "${lib.name}"?`)) {
                        try {
                          await api.deleteLibrary(lib.id);
                          if (selectedLibrary === lib.id) setSelectedLibrary(null);
                          loadData();
                        } catch (err: any) {
                          toast.error('Failed: ' + err.message);
                        }
                      }
                    }}
                    className="flex-1 sm:flex-none flex items-center justify-center gap-2 px-4 py-2 bg-zinc-900 hover:bg-red-950 text-red-500 rounded-xl text-xs font-black uppercase tracking-widest transition border border-zinc-800 hover:border-red-900/50"
                  >
                    <Trash2 className="w-4 h-4" /> Remove
                  </button>
                </div>
              </div>
            ))}
          </div>
        </div>
      </div>
    </div>
  );
};

export default SettingsPage;
