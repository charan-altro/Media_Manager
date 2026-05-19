import { renderHook, act } from '@testing-library/react';
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { useVideoPlayer } from '../useVideoPlayer';
import type Player from 'video.js/dist/types/player';

describe('useVideoPlayer', () => {
  let mockPlayer: any;

  beforeEach(() => {
    mockPlayer = {
      currentTime: vi.fn(),
      duration: vi.fn(),
      paused: vi.fn(),
      play: vi.fn(),
      pause: vi.fn(),
      muted: vi.fn(),
      isDisposed: vi.fn().mockReturnValue(false),
    };
  });

  afterEach(() => {
    vi.clearAllMocks();
  });

  it('should seek forward/backward correctly', () => {
    mockPlayer.currentTime.mockReturnValue(10);
    const { result } = renderHook(() => useVideoPlayer(mockPlayer as Player));

    act(() => {
      result.current.seekStep(5);
    });
    expect(mockPlayer.currentTime).toHaveBeenCalledWith(15);

    act(() => {
      result.current.seekStep(-5);
    });
    expect(mockPlayer.currentTime).toHaveBeenCalledWith(5);
  });

  it('should seek percentage correctly', () => {
    mockPlayer.duration.mockReturnValue(100);
    const { result } = renderHook(() => useVideoPlayer(mockPlayer as Player));

    act(() => {
      result.current.seekPercent(50);
    });
    expect(mockPlayer.currentTime).toHaveBeenCalledWith(50);
  });

  it('should handle hotkeys', () => {
    mockPlayer.currentTime.mockReturnValue(10);
    mockPlayer.duration.mockReturnValue(100);
    mockPlayer.paused.mockReturnValue(true);
    mockPlayer.muted.mockReturnValue(false);

    renderHook(() => useVideoPlayer(mockPlayer as Player));

    // Space to play
    window.dispatchEvent(new KeyboardEvent('keydown', { key: ' ' }));
    expect(mockPlayer.play).toHaveBeenCalled();

    // 'm' to mute
    window.dispatchEvent(new KeyboardEvent('keydown', { key: 'm' }));
    expect(mockPlayer.muted).toHaveBeenCalledWith(true);

    // ArrowRight to seek 5s
    window.dispatchEvent(new KeyboardEvent('keydown', { key: 'ArrowRight' }));
    expect(mockPlayer.currentTime).toHaveBeenCalledWith(15);

    // Shift + ArrowRight to seek 10s
    window.dispatchEvent(new KeyboardEvent('keydown', { key: 'ArrowRight', shiftKey: true }));
    expect(mockPlayer.currentTime).toHaveBeenCalledWith(20);

    // Ctrl + ArrowRight to seek 60s
    window.dispatchEvent(new KeyboardEvent('keydown', { key: 'ArrowRight', ctrlKey: true }));
    expect(mockPlayer.currentTime).toHaveBeenCalledWith(70);

    // '1' to seek 10%
    window.dispatchEvent(new KeyboardEvent('keydown', { key: '1' }));
    expect(mockPlayer.currentTime).toHaveBeenCalledWith(10);
  });

  it('should not trigger hotkeys when typing in input fields', () => {
    renderHook(() => useVideoPlayer(mockPlayer as Player));

    const input = document.createElement('input');
    document.body.appendChild(input);
    input.focus();

    window.dispatchEvent(new KeyboardEvent('keydown', { key: ' ', bubbles: true }));
    expect(mockPlayer.play).not.toHaveBeenCalled();

    document.body.removeChild(input);
  });
});
