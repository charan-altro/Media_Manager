import { useState } from 'react';
import { api } from '../api/adapter';
import toast from 'react-hot-toast';

export function useMediaActions() {
  const [refreshingIds, setRefreshingIds] = useState<Record<number, boolean>>({});

  const handleRefreshMetadata = async (id: number) => {
    if (refreshingIds[id]) return;
    setRefreshingIds(prev => ({ ...prev, [id]: true }));
    try {
      await api.refreshMetadata(id);
    } catch (err) {
      console.error('Failed to refresh metadata', err);
      setRefreshingIds(prev => ({ ...prev, [id]: false }));
    }
  };

  const handleProcessAdvanced = async (id: number, type: 'movie' | 'tv' = 'movie') => {
    if (refreshingIds[id]) return;
    setRefreshingIds(prev => ({ ...prev, [id]: true }));
    try {
      if (type === 'movie') {
        await api.processMovieAdvanced(id);
      } else {
        await api.processTvShowAdvanced(id);
      }
      toast.success('Advanced analysis started in background.');
    } catch (err) {
      console.error('Failed to start advanced analysis', err);
      setRefreshingIds(prev => ({ ...prev, [id]: false }));
    }
  };

  return {
    refreshingIds,
    setRefreshingIds,
    handleRefreshMetadata,
    handleProcessAdvanced,
  };
}