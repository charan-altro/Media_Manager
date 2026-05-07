import React from 'react';
import { Activity, Clock, Cpu } from 'lucide-react';

interface TasksPageProps {
  tasks: any[];
  currentTime: number;
}

const TasksPage: React.FC<TasksPageProps> = ({ tasks, currentTime }) => {
  return (
    <div className="px-4 md:px-12 py-24 min-h-screen">
      <div className="max-w-4xl mx-auto space-y-8">
        <div>
          <h3 className="text-4xl font-black text-white uppercase italic tracking-tighter">System Activity</h3>
          <p className="text-zinc-500 font-medium mt-2">Monitor background processes and library maintenance.</p>
        </div>
        
        <div className="grid gap-4">
          {tasks.length === 0 && (
            <div className="bg-zinc-900/30 border border-zinc-800 rounded-2xl p-12 text-center space-y-4">
              <Activity className="w-12 h-12 text-zinc-700 mx-auto" />
              <p className="text-zinc-500 font-bold uppercase tracking-widest text-xs">No active tasks</p>
            </div>
          )}
          {tasks.map(task => {
            const percentage = task.total > 0 ? Math.round((task.progress / task.total) * 100) : 0;
            const duration = task.startedAt ? Math.round((currentTime - task.startedAt) / 1000) : null;

            return (
              <div key={task.taskId} className="bg-[#181818] p-6 rounded-2xl border border-zinc-800 shadow-xl space-y-4">
                <div className="flex justify-between items-center">
                  <div className="flex items-center gap-4">
                    <div className={`p-2 rounded-lg ${task.status === 'completed' ? 'bg-green-500/10' : task.status === 'error' ? 'bg-red-500/10' : 'bg-blue-500/10'}`}>
                      <Activity className={`w-5 h-5 ${task.status === 'completed' ? 'text-green-500' : task.status === 'error' ? 'text-red-500' : 'text-blue-500 animate-pulse'}`} />
                    </div>
                    <div>
                      <div className="text-white font-black uppercase italic tracking-tight">{task.message}</div>
                      <div className="flex items-center gap-2 text-[10px] text-zinc-500 font-bold uppercase tracking-widest">
                        <span>{task.status}</span>
                        {duration !== null && (
                          <>
                            <span className="text-zinc-700">•</span>
                            <span className="flex items-center gap-1">
                              <Clock className="w-3 h-3" /> {duration}s elapsed
                            </span>
                          </>
                        )}
                      </div>
                    </div>
                  </div>
                  <div className="flex flex-col items-end gap-1">
                    <div className="text-xs font-black font-mono text-zinc-400 bg-zinc-900 px-3 py-1 rounded-full border border-zinc-800">
                      {task.progress} / {task.total} ({percentage}%)
                    </div>
                  </div>
                </div>

                <div className="h-1.5 w-full bg-zinc-900 rounded-full overflow-hidden border border-zinc-800">
                  <div
                    className={`h-full transition-all duration-500 ${
                      task.status === 'completed' ? 'bg-green-500' : 
                      task.status === 'error' ? 'bg-red-600' : 
                      'bg-blue-600 shadow-[0_0_15px_rgba(37,99,235,0.5)]'
                    }`}
                    style={{ width: `${percentage}%` }}
                  />
                </div>

                {task.debugInfo && (
                  <div className="bg-black/40 p-3 rounded-lg border border-zinc-800/50">
                    <div className="flex items-center gap-2 text-[10px] text-zinc-600 font-bold uppercase tracking-widest mb-1">
                      <Cpu className="w-3 h-3" /> Debug Information
                    </div>
                    <div className="text-[10px] font-mono text-zinc-500 break-all">
                      {task.debugInfo}
                    </div>
                  </div>
                )}
              </div>
            );
          })}
        </div>
      </div>
    </div>
  );
};

export default TasksPage;
