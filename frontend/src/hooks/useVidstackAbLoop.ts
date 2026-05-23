import { useState, useCallback } from 'react';

export interface AbLoopManager {
  loopStart: number | null;
  loopEnd: number | null;
  loopEnabled: boolean;
  setStart: () => void;
  setEnd: () => void;
  toggleLoop: () => void;
  clearLoop: () => void;
  checkLoop: (currentTime: number, seek: (time: number) => void) => void;
  setCurrentTimeForControls: (time: number) => void;
}

export const useVidstackAbLoop = (): AbLoopManager => {
  const [loopStart, setLoopStart] = useState<number | null>(null);
  const [loopEnd, setLoopEnd] = useState<number | null>(null);
  const [loopEnabled, setLoopEnabled] = useState(false);
  
  // Track current time purely for the Set A / Set B buttons, updated via checkLoop
  const [localCurrentTime, setLocalCurrentTime] = useState(0);

  const toggleLoop = useCallback(() => {
    if (loopStart !== null && loopEnd !== null) {
      setLoopEnabled(prev => !prev);
    }
  }, [loopStart, loopEnd]);

  const setStart = useCallback(() => {
    setLoopStart(localCurrentTime);
    if (loopEnd !== null && localCurrentTime >= loopEnd) {
      setLoopEnd(null);
      setLoopEnabled(false);
    }
  }, [localCurrentTime, loopEnd]);

  const setEnd = useCallback(() => {
    if (loopStart !== null && localCurrentTime > loopStart) {
      setLoopEnd(localCurrentTime);
      setLoopEnabled(true);
    }
  }, [localCurrentTime, loopStart]);

  const clearLoop = useCallback(() => {
    setLoopStart(null);
    setLoopEnd(null);
    setLoopEnabled(false);
  }, []);

  // Called rapidly by onTimeUpdate in the player
  const checkLoop = useCallback((currentTime: number, seek: (time: number) => void) => {
    setLocalCurrentTime(currentTime);
    if (loopEnabled && loopStart !== null && loopEnd !== null) {
      if (currentTime >= loopEnd) {
        seek(loopStart);
      }
    }
  }, [loopEnabled, loopStart, loopEnd]);

  return {
    loopStart,
    loopEnd,
    loopEnabled,
    setStart,
    setEnd,
    toggleLoop,
    clearLoop,
    checkLoop,
    setCurrentTimeForControls: setLocalCurrentTime
  };
};

