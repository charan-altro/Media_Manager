import { useEffect, useCallback } from 'react';
import type { AbLoopControls } from './useAbLoop';

export const useVideoPlayer = (player: any, abLoop?: AbLoopControls) => {
  const p = player;

  const seekStep = useCallback((seconds: number) => {
    if (!p) return;
    const currentTime = p.currentTime() || 0;
    p.currentTime(currentTime + seconds);
  }, [p]);

  const seekPercent = useCallback((percent: number) => {
    if (!p) return;
    const duration = p.duration();
    if (duration) {
      p.currentTime(duration * (percent / 100));
    }
  }, [p]);

  const handleKeyDown = useCallback((event: KeyboardEvent) => {
    if (!p) return;

    // Avoid triggering hotkeys when typing in input fields
    const target = event.target as HTMLElement;
    if (target.tagName === 'INPUT' || target.tagName === 'TEXTAREA' || target.isContentEditable) {
      return;
    }

    switch (event.key) {
      case ' ':
        event.preventDefault(); // Prevent page scroll
        if (p.paused()) {
          p.play();
        } else {
          p.pause();
        }
        break;
      case 'm':
      case 'M':
        p.muted(!p.muted());
        break;
      case 'ArrowRight': {
        let step = 5;
        if (event.shiftKey) step = 10;
        if (event.ctrlKey || event.metaKey) step = 60;
        seekStep(step);
        break;
      }
      case 'ArrowLeft': {
        let step = -5;
        if (event.shiftKey) step = -10;
        if (event.ctrlKey || event.metaKey) step = -60;
        seekStep(step);
        break;
      }
      case '[':
        seekPercent((((p.currentTime() || 0) / (p.duration() || 1)) * 100) - 10);
        break;
      case ']':
        seekPercent((((p.currentTime() || 0) / (p.duration() || 1)) * 100) + 10);
        break;
      case 'l':
      case 'L':
        if (abLoop) abLoop.toggleLoop();
        break;
      case 'a':
      case 'A':
        if (abLoop) {
          abLoop.setStart();
          p.currentTime(p.currentTime()); // Set start and go to start/keep current position
        }
        break;
      case 'b':
      case 'B':
        if (abLoop) abLoop.setEnd();
        break;
      default:
        if (/^[0-9]$/.test(event.key)) {
          seekPercent(parseInt(event.key, 10) * 10);
        }
        break;
    }
  }, [p, seekStep, seekPercent, abLoop]);

  useEffect(() => {
    window.addEventListener('keydown', handleKeyDown);
    return () => {
      window.removeEventListener('keydown', handleKeyDown);
    };
  }, [handleKeyDown]);

  return {
    seekStep,
    seekPercent,
  };
};

