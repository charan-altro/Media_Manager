import { useState, useEffect, useCallback } from 'react';

export interface AbLoopControls {
  loopStart: number | null;
  loopEnd: number | null;
  loopEnabled: boolean;
  setStart: () => void;
  setEnd: () => void;
  toggleLoop: () => void;
}

export const useAbLoop = (player: any): AbLoopControls => {
  const [loopStart, setLoopStart] = useState<number | null>(null);
  const [loopEnd, setLoopEnd] = useState<number | null>(null);
  const [loopEnabled, setLoopEnabled] = useState(false);

  // Monitor time update to enforce loop
  useEffect(() => {
    if (!player || !loopEnabled || loopStart === null || loopEnd === null) return;

    const handleTimeUpdate = () => {
      const currentTime = player.currentTime;
      if (currentTime >= loopEnd) {
        player.currentTime = loopStart;
      }
    };

    player.addEventListener('timeupdate', handleTimeUpdate);
    return () => {
      player.removeEventListener('timeupdate', handleTimeUpdate);
    };
  }, [player, loopStart, loopEnd, loopEnabled]);

  const toggleLoop = useCallback(() => {
    setLoopEnabled(prev => !prev);
  }, []);

  const setStart = useCallback(() => {
    if (!player) return;
    const time = player.currentTime;
    setLoopStart(time);
    if (loopEnd !== null && time >= loopEnd) {
      setLoopEnd(null);
    }
  }, [player, loopEnd]);

  const setEnd = useCallback(() => {
    if (!player) return;
    const time = player.currentTime;
    if (loopStart !== null && time > loopStart) {
      setLoopEnd(time);
      setLoopEnabled(true);
    }
  }, [player, loopStart]);

  return {
    loopStart,
    loopEnd,
    loopEnabled,
    setStart,
    setEnd,
    toggleLoop
  };
};
