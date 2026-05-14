import React, { createContext, useContext, useState, useEffect, useRef } from 'react';
import type { ReactNode } from 'react';
import { api, API_BASE, IS_TAURI, type TaskUpdate } from '../api/adapter';
import { listen } from '@tauri-apps/api/event';
import toast from 'react-hot-toast';

interface TaskContextType {
  tasks: Record<string, TaskUpdate>;
  runningTasks: TaskUpdate[];
  latestTask: TaskUpdate | undefined;
}

const TaskContext = createContext<TaskContextType | undefined>(undefined);

export const TaskProvider: React.FC<{ children: ReactNode; loadData: () => void; setRefreshingIds: (ids: any) => void }> = ({ children, loadData, setRefreshingIds }) => {
  const [tasks, setTasks] = useState<Record<string, TaskUpdate>>({});
  const taskUpdatesQueue = useRef<Record<string, TaskUpdate>>({});
  const lastFlushTime = useRef<number>(0);
  const flushTimeout = useRef<any>(null);

  const flushTaskUpdates = () => {
    const updates = { ...taskUpdatesQueue.current };
    if (Object.keys(updates).length === 0) return;
    
    taskUpdatesQueue.current = {};
    setTasks(prev => {
      const newTasks = { ...prev };
      Object.entries(updates).forEach(([id, update]) => {
        const oldStatus = prev[id]?.status;

        if (update.status === 'error' && oldStatus !== 'error') {
          toast.error(`Task Failed: ${update.message}`, { duration: 5000 });
        } else if (update.status === 'completed' && oldStatus !== 'completed') {
          toast.success(`Task Completed: ${update.message}`, { duration: 5000 });
          setRefreshingIds({}); 
          setTimeout(loadData, 1000);
        }
        newTasks[id] = update;
      });
      return newTasks;
    });
    lastFlushTime.current = Date.now();
  };

  const handleTaskUpdate = (update: any) => {
    const normalizedUpdate: TaskUpdate = {
      ...update,
      taskId: update.taskId || update.task_id,
      startedAt: update.startedAt || update.started_at,
      finishedAt: update.finishedAt || update.finished_at,
      debugInfo: update.debugInfo || update.debug_info,
      filesNew: update.filesNew || update.files_new || 0,
      filesHealed: update.filesHealed || update.files_healed || 0,
      filesMissing: update.filesMissing || update.files_missing || 0,
    };

    taskUpdatesQueue.current[normalizedUpdate.taskId] = normalizedUpdate;

    const now = Date.now();
    const timeSinceLastFlush = now - lastFlushTime.current;
    
    if (timeSinceLastFlush > 500) {
      if (flushTimeout.current) clearTimeout(flushTimeout.current);
      flushTaskUpdates();
    } else if (!flushTimeout.current) {
      flushTimeout.current = setTimeout(() => {
        flushTimeout.current = null;
        flushTaskUpdates();
      }, 500 - timeSinceLastFlush);
    }
  };

  useEffect(() => {
    // Initial fetch
    api.getTasks().then(initialTasks => {
      const initialMap: Record<string, TaskUpdate> = {};
      initialTasks.forEach((t: any) => {
        const normalized = {
          ...t,
          taskId: t.taskId || t.task_id,
          startedAt: t.startedAt || t.started_at,
          finishedAt: t.finishedAt || t.finished_at,
          debugInfo: t.debugInfo || t.debug_info,
          filesNew: t.filesNew || t.files_new || 0,
          filesHealed: t.filesHealed || t.files_healed || 0,
          filesMissing: t.filesMissing || t.files_missing || 0,
        };
        initialMap[normalized.taskId] = normalized;
      });
      setTasks(initialMap);
    }).catch(err => console.error('Failed to fetch initial tasks:', err));

    let cleanup: () => void = () => {};

    if (IS_TAURI) {
      const unlistenPromise = listen<TaskUpdate>('task-update', (event) => {
        handleTaskUpdate(event.payload);
      });
      cleanup = () => { unlistenPromise.then(unlisten => unlisten()); };
    } else {
      const eventSource = new EventSource(`${API_BASE}/tasks/stream`);
      eventSource.onmessage = (event) => {
        handleTaskUpdate(JSON.parse(event.data));
      };
      cleanup = () => eventSource.close();
    }

    return () => {
      cleanup();
      if (flushTimeout.current) clearTimeout(flushTimeout.current);
    };
  }, []);

  const runningTasks = Object.values(tasks).filter(t => t.status === 'running');
  const latestTask = runningTasks[runningTasks.length - 1];

  return (
    <TaskContext.Provider value={{ tasks, runningTasks, latestTask }}>
      {children}
    </TaskContext.Provider>
  );
};

export const useTasks = () => {
  const context = useContext(TaskContext);
  if (context === undefined) {
    throw new Error('useTasks must be used within a TaskProvider');
  }
  return context;
};
