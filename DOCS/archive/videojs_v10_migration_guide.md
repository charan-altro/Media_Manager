# Migration Plan: Video.js v8 to v10 (DEFERRED)

> [!NOTE]
> **Deferred / Archived**: This migration proposal was deferred. The project continues to use the **Vidstack** player library, which is fully implemented and optimized.

**Status:** Draft  
**Target Version:** Video.js `v10.0.0-beta.23`  

This document outlines the step-by-step roadmap to migrate the Media Manager's frontend video player from Video.js v8 to Video.js v10. Since v10 introduces a modular, declarative React architecture, legacy monolithic plugins are replaced with native React components and hooks.

---

## Phase 1: Dependency Upgrades

Update [package.json](file:///c:/Users/chara/New_Projects_antigravity/Media_Manager-dev/Media_Manager/frontend/package.json) to remove the monolithic packages and add the scoped v10 modules.

### Steps:
1. **Uninstall legacy packages:**
   ```bash
   npm uninstall video.js videojs-abloop @types/video.js
   ```
2. **Install modular v10 modules:**
   ```bash
   npm install @videojs/core@10.0.0-beta.23 @videojs/react@10.0.0-beta.23
   ```

---

## Phase 2: Refactoring A-B Looping to React

Because `videojs-abloop` will not function in v10, you can implement the looping logic directly inside a custom React hook/component. This removes third-party dependencies.

### Implementation Pattern (`useAbLoop.ts`):
```typescript
import { useState, useEffect, useCallback } from 'react';

export const useAbLoop = (player: any) => {
  const [loopStart, setLoopStart] = useState<number | null>(null);
  const [loopEnd, setLoopEnd] = useState<number | null>(null);
  const [loopEnabled, setLoopEnabled] = useState(false);

  // Monitor time update to enforce loop
  useEffect(() => {
    if (!player || !loopEnabled || loopStart === null || loopEnd === null) return;

    const handleTimeUpdate = () => {
      const currentTime = player.currentTime();
      if (currentTime >= loopEnd) {
        player.currentTime(loopStart);
      }
    };

    player.on('timeupdate', handleTimeUpdate);
    return () => {
      player.off('timeupdate', handleTimeUpdate);
    };
  }, [player, loopStart, loopEnd, loopEnabled]);

  const toggleLoop = useCallback(() => {
    setLoopEnabled(prev => !prev);
  }, []);

  const setStart = useCallback(() => {
    if (!player) return;
    const time = player.currentTime();
    setLoopStart(time);
    if (loopEnd !== null && time >= loopEnd) {
      setLoopEnd(null);
    }
  }, [player, loopEnd]);

  const setEnd = useCallback(() => {
    if (!player) return;
    const time = player.currentTime();
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
```

---

## Phase 3: Converting Source Selector to a React UI Overlay

Instead of using Video.js menus to append a source selector, render a custom UI control overlay inside React. This fits Tailwind seamlessly.

### Steps:
1. **Remove `sourceSelector.ts` registration** from [VideoPlayer.tsx](file:///c:/Users/chara/New_Projects_antigravity/Media_Manager-dev/Media_Manager/frontend/src/components/VideoPlayer.tsx).
2. **Build a custom React dropdown UI** positioned absolutely over the control bar container.
3. Hook the dropdown choice back into the player’s source switcher handler.

---

## Phase 4: Adapting `VideoPlayer.tsx` to `@videojs/react`

Update the main player to use v10’s declarative wrapper instead of manual DOM insertion.

### v8 Pattern (Before):
```tsx
const videoElement = document.createElement("video-js");
videoRef.current.appendChild(videoElement);
const p = videojs(videoElement, options);
```

### v10 Pattern (After):
```tsx
import '@videojs/react/video/skin.css';
import { createPlayer, videoFeatures } from '@videojs/react';
import { VideoSkin, Video } from '@videojs/react/video';

const Player = createPlayer({ features: videoFeatures });

// Inside VideoPlayer:
return (
  <Player.Provider>
    <VideoSkin poster={posterUrl}>
      <Video 
        src={currentUrl} 
        autoplay 
        controls 
        onPlaying={handlePlaying}
        onPause={handlePause}
        onTimeUpdate={handleTimeUpdate}
      />
    </VideoSkin>
    
    {/* Custom React Overlays (Source selector, loops indicators) */}
    <CustomControlsOverlay player={player} />
  </Player.Provider>
);
```

---

## Phase 5: Updating Hotkeys and VTT Thumbnails

1. **Hotkeys:** Update [useVideoPlayer.ts](file:///c:/Users/chara/New_Projects_antigravity/Media_Manager-dev/Media_Manager/frontend/src/hooks/useVideoPlayer.ts) to read loop state from the React A-B loop state hook rather than querying `player.abLoopPlugin`.
2. **Thumbnails:** Update [VttThumbnails.tsx](file:///c:/Users/chara/New_Projects_antigravity/Media_Manager-dev/Media_Manager/frontend/src/components/VttThumbnails.tsx) to target the modern v10 control bar layout structure to calculate bounds on hover.
