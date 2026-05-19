import { useEffect, useState, useCallback, useRef } from 'react';
import { useMediaRemote, useMediaState } from '@vidstack/react';

export interface AbLoopManager {
  loopStart: number | null;
  loopEnd: number | null;
  loopEnabled: boolean;
  setStart: () => void;
  setEnd: () => void;
  toggleLoop: () => void;
  clearLoop: () => void;
}

export const useVidstackAbLoop = (): AbLoopManager => {
  const remote = useMediaRemote();
  const currentTime = useMediaState('currentTime');
  
  const [loopStart, setLoopStart] = useState<number | null>(null);
  const [loopEnd, setLoopEnd] = useState<number | null>(null);
  const [loopEnabled, setLoopEnabled] = useState(false);

  // Monitor time update to enforce loop
  useEffect(() => {
    if (loopEnabled && loopStart !== null && loopEnd !== null) {
      if (currentTime >= loopEnd) {
        remote.seek(loopStart);
      }
    }
  }, [currentTime, loopStart, loopEnd, loopEnabled, remote]);

  const toggleLoop = useCallback(() => {
    if (loopStart !== null && loopEnd !== null) {
      setLoopEnabled(prev => !prev);
    }
  }, [loopStart, loopEnd]);

  const setStart = useCallback(() => {
    setLoopStart(currentTime);
    if (loopEnd !== null && currentTime >= loopEnd) {
      setLoopEnd(null);
      setLoopEnabled(false);
    }
  }, [currentTime, loopEnd]);

  const setEnd = useCallback(() => {
    if (loopStart !== null && currentTime > loopStart) {
      setLoopEnd(currentTime);
      setLoopEnabled(true);
    }
  }, [currentTime, loopStart]);

  const clearLoop = useCallback(() => {
    setLoopStart(null);
    setLoopEnd(null);
    setLoopEnabled(false);
  }, []);

  return {
    loopStart,
    loopEnd,
    loopEnabled,
    setStart,
    setEnd,
    toggleLoop,
    clearLoop
  };
};
