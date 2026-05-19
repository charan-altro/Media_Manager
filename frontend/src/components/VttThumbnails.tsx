import React, { useEffect, useState, useRef, useCallback } from 'react';

interface VttThumbnailsProps {
  player: any; // Using any for compatibility with v10 for now
  vttUrl?: string;
}

interface ThumbnailData {
  startTime: number;
  endTime: number;
  image: string;
  x: number;
  y: number;
  w: number;
  h: number;
}

const formatTime = (seconds: number) => {
  const h = Math.floor(seconds / 3600);
  const m = Math.floor((seconds % 3600) / 60);
  const s = Math.floor(seconds % 60);
  
  if (h > 0) {
    return `${h}:${m.toString().padStart(2, '0')}:${s.toString().padStart(2, '0')}`;
  }
  return `${m}:${s.toString().padStart(2, '0')}`;
};

const MOCK_THUMBNAILS: ThumbnailData[] = Array.from({ length: 100 }, (_, i) => ({
  startTime: i * 10,
  endTime: (i + 1) * 10,
  image: `https://placehold.co/160x90/${(i % 10).toString(16).repeat(6)}/FFFFFF/png?text=Preview+${i + 1}`,
  x: 0,
  y: 0,
  w: 160,
  h: 90
}));

const FALLBACK_THUMB_IMAGE = 'https://placehold.co/160x90/333333/FFFFFF/png?text=Preview+Unavailable';

const VttThumbnails: React.FC<VttThumbnailsProps> = ({ player }) => {
  const [visible, setVisible] = useState(false);
  const [position, setPosition] = useState({ x: 0, y: 0 });
  const [time, setTime] = useState(0);
  const [currentThumb, setCurrentThumb] = useState<ThumbnailData | null>(null);
  const containerRef = useRef<HTMLDivElement>(null);

  const handleMouseMove = useCallback((e: MouseEvent, progressEl: HTMLElement) => {
    if (!player) return;
    const rect = progressEl.getBoundingClientRect();
    const x = e.clientX - rect.left;
    const percentage = Math.max(0, Math.min(1, x / rect.width));
    const duration = player.duration() || 0;
    const hoverTime = percentage * duration;

    setTime(hoverTime);
    
    const thumb = MOCK_THUMBNAILS.find(t => hoverTime >= t.startTime && hoverTime < t.endTime) || {
      startTime: 0,
      endTime: 0,
      image: FALLBACK_THUMB_IMAGE,
      x: 0, y: 0, w: 160, h: 90
    };
    
    setCurrentThumb(thumb);

    const thumbWidth = thumb.w;
    const halfWidth = thumbWidth / 2;
    
    let posX = e.clientX;
    if (posX - halfWidth < 10) posX = halfWidth + 10;
    if (posX + halfWidth > window.innerWidth - 10) posX = window.innerWidth - halfWidth - 10;

    setPosition({
      x: posX,
      y: rect.top - 10
    });
  }, [player]);

  useEffect(() => {
    if (!player) return;

    // In v10, we might need to wait for the player to be ready and the DOM to be rendered
    const findProgressEl = () => {
      const el = player.el();
      if (!el) return null;
      return el.querySelector('.vjs-progress-control') as HTMLElement;
    };

    let progressEl = findProgressEl();
    
    const setupListeners = (el: HTMLElement) => {
      const onMouseMove = (e: MouseEvent) => handleMouseMove(e, el);
      const onMouseEnter = () => setVisible(true);
      const onMouseLeave = () => setVisible(false);

      el.addEventListener('mousemove', onMouseMove);
      el.addEventListener('mouseenter', onMouseEnter);
      el.addEventListener('mouseleave', onMouseLeave);

      return () => {
        el.removeEventListener('mousemove', onMouseMove);
        el.removeEventListener('mouseenter', onMouseEnter);
        el.removeEventListener('mouseleave', onMouseLeave);
      };
    };

    let cleanup: (() => void) | undefined;

    if (progressEl) {
      cleanup = setupListeners(progressEl);
    } else {
      // Retry after a short delay if not found immediately (v10 React rendering delay)
      const timeout = setTimeout(() => {
        progressEl = findProgressEl();
        if (progressEl) {
          cleanup = setupListeners(progressEl);
        }
      }, 500);
      return () => clearTimeout(timeout);
    }

    return () => {
      if (cleanup) cleanup();
    };
  }, [player, handleMouseMove]);

  if (!visible) return null;

  return (
    <div 
      ref={containerRef}
      className="fixed z-[300] pointer-events-none -translate-x-1/2 -translate-y-full flex flex-col items-center gap-2"
      style={{ 
        left: position.x,
        top: position.y
      }}
    >
      <div className="bg-black border border-white/20 rounded shadow-2xl overflow-hidden">
        {currentThumb && (
          <div 
            style={{
              width: currentThumb.w,
              height: currentThumb.h,
              backgroundImage: `url(${currentThumb.image})`,
              backgroundPosition: `-${currentThumb.x}px -${currentThumb.y}px`,
              backgroundSize: 'cover'
            }}
          />
        )}
      </div>
      <div className="bg-black/80 backdrop-blur-md px-2 py-0.5 rounded text-[10px] font-mono text-white border border-white/10">
        {formatTime(time)}
      </div>
    </div>
  );
};

export default VttThumbnails;
