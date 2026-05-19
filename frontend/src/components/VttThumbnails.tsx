import React, { useEffect, useState, useRef } from 'react';
import type Player from 'video.js/dist/types/player';

interface VttThumbnailsProps {
  player: Player | null;
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

/**
 * Utility to format seconds into HH:MM:SS or MM:SS
 */
const formatTime = (seconds: number) => {
  const h = Math.floor(seconds / 3600);
  const m = Math.floor((seconds % 3600) / 60);
  const s = Math.floor(seconds % 60);
  
  if (h > 0) {
    return `${h}:${m.toString().padStart(2, '0')}:${s.toString().padStart(2, '0')}`;
  }
  return `${m}:${s.toString().padStart(2, '0')}`;
};

/**
 * Mock data moved outside component to avoid re-creation on every render.
 */
const MOCK_THUMBNAILS: ThumbnailData[] = [
  { startTime: 0, endTime: 10, image: 'https://placehold.co/160x90/000000/FFFFFF/png?text=Preview+1', x: 0, y: 0, w: 160, h: 90 },
  { startTime: 10, endTime: 20, image: 'https://placehold.co/160x90/111111/FFFFFF/png?text=Preview+2', x: 0, y: 0, w: 160, h: 90 },
  { startTime: 20, endTime: 30, image: 'https://placehold.co/160x90/222222/FFFFFF/png?text=Preview+3', x: 0, y: 0, w: 160, h: 90 },
];

const FALLBACK_THUMB_IMAGE = 'https://placehold.co/160x90/333333/FFFFFF/png?text=Preview+Unavailable';

const VttThumbnails: React.FC<VttThumbnailsProps> = ({ player }) => {
  const [visible, setVisible] = useState(false);
  const [position, setPosition] = useState({ x: 0, y: 0 });
  const [time, setTime] = useState(0);
  const [currentThumb, setCurrentThumb] = useState<ThumbnailData | null>(null);
  const containerRef = useRef<HTMLDivElement>(null);

  /**
   * Placeholder for future VTT parsing logic
   */
  const _parseVtt = async (url: string) => {
    try {
      const response = await fetch(url);
      const text = await response.text();
      console.log('VTT Content:', text.substring(0, 100));
    } catch (err) {
      console.error('Failed to parse VTT:', err);
    }
  };

  useEffect(() => {
    if (!player) return;

    const controlBar = player.getChild('controlBar');
    if (!controlBar) return;
    
    const progressControl = controlBar.getChild('progressControl');
    if (!progressControl) return;

    const el = progressControl.el();
    if (!el) return;

    const handleMouseMove = (e: Event) => {
      const mouseEvent = e as MouseEvent;
      const rect = el.getBoundingClientRect();
      const x = mouseEvent.clientX - rect.left;
      const percentage = Math.max(0, Math.min(1, x / rect.width));
      const duration = player.duration() || 0;
      const hoverTime = percentage * duration;

      setTime(hoverTime);
      
      // Find appropriate thumbnail (mock logic)
      const thumb = MOCK_THUMBNAILS.find(t => hoverTime >= t.startTime && hoverTime < t.endTime) || {
        startTime: 0,
        endTime: 0,
        image: FALLBACK_THUMB_IMAGE,
        x: 0, y: 0, w: 160, h: 90
      };
      
      setCurrentThumb(thumb);

      // Calculate position for the preview box
      const thumbWidth = thumb.w;
      const halfWidth = thumbWidth / 2;
      
      // Clamp x position within window
      let posX = mouseEvent.clientX;
      if (posX - halfWidth < 10) posX = halfWidth + 10;
      if (posX + halfWidth > window.innerWidth - 10) posX = window.innerWidth - halfWidth - 10;

      setPosition({
        x: posX,
        y: rect.top - 10
      });
    };

    const handleMouseEnter = () => setVisible(true);
    const handleMouseLeave = () => setVisible(false);

    el.addEventListener('mousemove', handleMouseMove as EventListener);
    el.addEventListener('mouseenter', handleMouseEnter as EventListener);
    el.addEventListener('mouseleave', handleMouseLeave as EventListener);

    return () => {
      el.removeEventListener('mousemove', handleMouseMove as EventListener);
      el.removeEventListener('mouseenter', handleMouseEnter as EventListener);
      el.removeEventListener('mouseleave', handleMouseLeave as EventListener);
    };
  }, [player, _parseVtt]);

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
