import { useEffect, useCallback } from 'react';
import type Player from 'video.js/dist/types/player';

interface AbLoopPlugin {
  setStart(time?: number): AbLoopPlugin;
  setEnd(time?: number): AbLoopPlugin;
  toggle(): AbLoopPlugin;
  goToStart(): AbLoopPlugin;
  enable(): AbLoopPlugin;
  disable(): AbLoopPlugin;
  getOptions(): { start: number; end: number; enabled: boolean };
}

interface VideoPlayerWithPlugins extends Player {
  abLoopPlugin?: AbLoopPlugin;
}

export const useVideoPlayer = (player: Player | null) => {
  const p = player as VideoPlayerWithPlugins;

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
        if (p.abLoopPlugin) p.abLoopPlugin.toggle();
        break;
      case 'a':
      case 'A':
        if (p.abLoopPlugin) p.abLoopPlugin.setStart().goToStart();
        break;
      case 'b':
      case 'B':
        if (p.abLoopPlugin) p.abLoopPlugin.setEnd();
        break;
      default:
        if (/^[0-9]$/.test(event.key)) {
          seekPercent(parseInt(event.key, 10) * 10);
        }
        break;
    }
  }, [p, seekStep, seekPercent]);

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
