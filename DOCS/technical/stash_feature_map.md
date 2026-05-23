# Stash Feature Map — Complete Reverse-Engineering Reference
> **Purpose:** Benchmark & parity analysis for the custom Rust Media Manager  
> **Source tree:** `c:\Users\chara\New_Projects_antigravity\stash_reference_project_GO`  
> **Last read:** 2026-05-16  

---

## 1. VIDEO PLAYER (Frontend — Video.js + custom plugins)

### 1.1 Basic Video Player
| Feature | File | Key code | Video.js v10 Equivalent |
|---------|------|----------|-------------------------|
| Video.js player init | `ScenePlayer.tsx:337–446` | `videojs(videoEl, options)` | Native: `createPlayer({ features: videoFeatures })` & `<Player.Provider>` |
| Responsive portrait/landscape detection | `ScenePlayer.tsx:976–977` | `file.height > file.width` → CSS class | Native: CSS flex/grid layout on `<Player.Container>` |
| Inline playback on mobile (`playsinline`) | `ScenePlayer.tsx:369` | `playsinline: true` | Native: `<Video playsInline />` prop |
| Poster / screenshot | `ScenePlayer.tsx:791` | `player.poster(scene.paths.screenshot)` | Native: `<Poster src={scene.paths.screenshot} />` component |
| Loop control (configurable max duration) | `ScenePlayer.tsx:288–295` | `looping = duration < maxLoopDuration` | Native: `<Video loop />` prop or React conditional ended listener |
| Playback rates | `ScenePlayer.tsx:366` | `playbackRates: [0.25, 0.5 … 2]` | Native: `<PlaybackRateButton>` component |
| Inactivity timeout (700ms) | `ScenePlayer.tsx:367` | `inactivityTimeout: 700` | Native: `Controls` feature auto-hide state on `<Player.Container>` |

---

### 1.2 Video Player with Seeking
| Feature | File | Key code | Video.js v10 Equivalent |
|---------|------|----------|-------------------------|
| Arrow-key step seek (10 / 5 / 60 s) | `ScenePlayer.tsx:59–114` | `seekStep(seekFactor)` | Native: `seek(currentTime ± step)` from Time feature |
| 0–9 hotkeys → jump to % | `ScenePlayer.tsx:164–193` | `seekPercent(0.1 … 0.9)` | Native: Custom key listener calling `seek((pct/100)*duration)` |
| `[` / `]` hotkeys → ±10% relative | `ScenePlayer.tsx:194–199` | `seekPercentRelative(±0.1)` | Native: Custom key listener calling `seek()` |
| Programmatic `setTimestamp` API | `ScenePlayer.tsx:320–333` | `player.currentTime(value)` | Native: `seek(value)` action from Time feature |
| Scrubber bar seek (visual sprite) | `ScenePlayerScrubber.tsx` | `onScrubberSeek(seconds)` | Native: `<TimeSlider>` component |
| Pause before scrub, resume after | `ScenePlayer.tsx:937–954` | `pausedBeforeScrubber.current` | Native: `<TimeSlider>` pointer events + Playback actions |
| Resume playback at saved position | `ScenePlayer.tsx:689–700` | `resumeTime = scene.resume_time` | Native: React state / hook calling `seek(resumeTime)` on load |
| Always-start-from-beginning toggle | `ScenePlayer.tsx:687` | `alwaysStartFromBeginning` | Native: Conditional logic in React initialization hook |
| A-B Loop plugin | `ScenePlayer.tsx:398–406` | `abLoopPlugin` (videojs-abloop) | Native: Custom React hook tracking `currentTime` and seeking back |
| Shift+L hotkey to toggle A-B loop | `ScenePlayer.tsx:117–119` | `toggleABLooping()` | Native: React custom hotkey mapping |
| Shift+L loop toggle in loop control | `ScenePlayer.tsx:116–119` | `player.loop(!player.loop())` | Native: React custom hotkey mapping |
| MediaSession "prev/next track" keys | `ScenePlayer.tsx:129–140` | `MediaTrackNext` / `MediaTrackPrevious` | Native: Custom hook calling browser's `navigator.mediaSession` |
| `onComplete` callback on video end | `ScenePlayer.tsx:909–915` | `player.on("ended", onComplete)` | Native: `<Video onEnded={onComplete} />` or store `ended` subscription |

---

### 1.3 Smart Streaming Logic (Source Selector)
| Feature | File | Key code | Video.js v10 Equivalent |
|---------|------|----------|-------------------------|
| Multi-source menu in control bar | `source-selector.ts:37–101` | `SourceMenuButton` added to controlBar | Native: Custom component subscribing to Source feature |
| Auto-advance to next source on error | `source-selector.ts:165–208` | `MEDIA_ERR_SRC_NOT_SUPPORTED` → try next | Native: Subscribing to `Error` feature and calling `setSource()` |
| Mark errored sources visually | `source-selector.ts:95–100` | `vjs-source-menu-item-error` CSS class | Native: Local state storing error flags mapped to source selector options |
| Preserve current time on source switch | `source-selector.ts:126–136` | `player.one("canplay", () => player.currentTime(currentTime))` | Native: React `useEffect` seeking back to cached `currentTime` post-source change |
| Skip Safari file-transcode sources | `ScenePlayer.tsx:625–629` | `!(isFileTranscode && isSafari)` | Native: Standard JS environment/Safari checks during stream setup |
| Label + MIME type per source | `ScenePlayer.tsx:631–641` | `{ src, type, label, offset, duration }` | Native: Handled declaratively in active `Source` state |
| Per-scene stream URL building | `scene.go:78–219` | `GetSceneStreamPaths()` produces all endpoints | Native: Same backend Rust design feeding frontend source list |

**Available stream types (backend-driven):**
- `Direct stream` — serve original file  
- `MP4` — live transcoded H.264/AAC fragmented MP4  
- `WEBM` — live transcoded VP9/Opus  
- `MKV` — copy-through for native MKV files  
- `HLS` — segmented MPEG-TS (`.m3u8`)  
- `DASH` — segmented WebM (`.mpd`)  
- All of the above at `Original / 4K / 1080p / 720p / 480p / 240p` resolutions

---

### 1.4 Video Player with Volume / Sound Bar
| Feature | File | Key code | Video.js v10 Equivalent |
|---------|------|----------|-------------------------|
| Volume panel (vertical, not inline) | `ScenePlayer.tsx:342–344` | `volumePanel: { inline: false }` | Native: Composable `<VolumeSlider>` and `<MuteButton>` components |
| Up/Down arrow volume ±0.1 | `ScenePlayer.tsx:158–163` | `player.volume(player.volume() ± 0.1)` | Native: Custom key listener calling `setVolume(volume ± 0.1)` |
| `M` hotkey mute/unmute | `ScenePlayer.tsx:148–150` | `player.muted(!player.muted())` | Native: Custom key listener calling `toggleMuted()` |
| Persist volume across sessions | `persist-volume.ts` | Custom videojs plugin `persistVolume` | Native: Custom React effect sync with `localStorage` |

---

### 1.5 Video Player with Subtitles / Captions
| Feature | File | Key code | Video.js v10 Equivalent |
|---------|------|----------|-------------------------|
| Caption discovery per scene | `routes_scene.go:458–467` | `GET /caption?lang=&type=` → WebVTT | Native: Mapped to standard backend API route |
| Multi-language caption loading | `ScenePlayer.tsx:658–685` | `sourceSelector.addTextTrack(...)` | Native: Declarative `<track>` elements nested under `<Video>` component |
| Auto-detect browser language | `ScenePlayer.tsx:644–655` | `window.navigator.language` | Native: React hook setting `default` flag dynamically on `<track>` items |
| Track label = language name + type | `ScenePlayer.tsx:663–670` | `languageMap.get(lang)` | Native: Populated as standard `label` attribute on `<track>` |
| Default track selection | `ScenePlayer.tsx:670–673` | `default: setAsDefault` | Native: Handled dynamically with `<track default />` prop |
| Convert SRT/ASS → WebVTT on-the-fly | `routes_scene.go:437–455` | `sub.WriteToWebVTT(&buf)` | Native: Handled on backend side (Rust media module conversion) |
| Background subtitle opacity styling | `ScenePlayer.tsx:422–427` | `textTrackSettings.setValues(...)` | Native: Stylable via CSS target `.vjs-text-track-cue` |
| Caption backend DB storage | `routes_scene.go:408–466` | `CaptionFinder.GetCaptions(ctx, fileID)` | Native: SQL repository queries mapping subtitle files (Rust backend) |

---

### 1.6 VTT Thumbnail Scrubber (Sprite Preview on Hover)
| Feature | File | Key code | Video.js v10 Equivalent |
|---------|------|----------|-------------------------|
| Parse sprite VTT → background offset | `vtt-thumbnails.ts:244–259` | `WebVTT.Parser` → `IVTTData[]` | Native: `<Thumbnail>` component handles seek preview parsing natively |
| Hover on progress bar to show preview | `vtt-thumbnails.ts:158–177` | `onBarPointerEnter/Move/Leave` | Native: Automatic when using `<TimeSlider>` with `<Thumbnail>` |
| Position clamp at edges | `vtt-thumbnails.ts:219–233` | `marginLeft / marginRight` | Native: Handled internally by `<Thumbnail>` layout |
| Timestamp display | `vtt-thumbnails.ts:13` | `showTimestamp: boolean` | Native: `<Time>` displayed inside tooltip overlay |
| VTT file served from backend | `routes_scene.go:352–364` | `GET /vtt/thumbs` → sprite VTT | Native: Rust backend VTT response (same design) |
| Sprite JPEG served separately | `routes_scene.go:366–377` | `GET /vtt/sprite` → sprite JPEG | Native: Rust backend static asset server (same design) |

---

### 1.7 Scene Markers on Progress Bar
| Feature | File | Key code |
|---------|------|----------|
| Dot markers at timestamp | `markers.ts:64–112` | `markerSet.dot` → CSS `vjs-marker` |
| Range markers (with start+end) | `markers.ts:129–197` | `rangeDiv` with `calc()` width |
| Non-overlapping layered ranges (MWIS DP) | `markers.ts:218–262` | `findMWIS()` → dynamic programming |
| Colour-coded by tag (SHA256 hue) | `markers.ts:297–408` | `computeBaseHue()` + `adjustHues()` |
| Click dot → jump to time | `markers.ts:83–84` | `player.currentTime(marker.seconds)` |
| Hover → tooltip with title | `markers.ts:50–62` | `showMarkerTooltip(title)` |
| VTT chapter file for seekbar | `routes_scene.go:310–350` | `GET /vtt/chapter` → WEBVTT format |

---

### 1.8 Interactive / Funscript Support (haptic devices)
| Feature | File | Key code |
|---------|------|----------|
| Upload funscript to device | `ScenePlayer.tsx:457–468` | `uploadScript(scene.paths.funscript)` |
| Sync play / pause / seek to device | `ScenePlayer.tsx:544–558` | `interactiveClient.play/pause()` |
| Double play trigger (video.js lag fix) | `ScenePlayer.tsx:549–552` | 1 000 ms delayed second `play()` |
| Funscript served as static file | `routes_scene.go:379–384` | `GET /funscript` |
| Convert funscript → CSV (TheHandy) | `routes_scene.go:386–398` | `ConvertFunscriptToCSV()` |
| Interactive heatmap image | `routes_scene.go:400–406` | `GET /interactive_heatmap` |

---

### 1.9 Additional Player Plugins
| Plugin | File | What it does | Video.js v10 Equivalent |
|--------|------|--------------|-------------------------|
| `bigButtons` | `big-buttons.ts` | Large central play/pause buttons | Native: Stylable `<PlayButton>` overlay |
| `seekButtons` | built-in (videojs-seek-buttons) | ±10s seek buttons in controlbar | Native: `<SeekButton>` component |
| `skipButtons` | `PlaylistButtons.ts` | Skip to next/previous scene | Native: Custom components using React handlers |
| `autostartButton` | `autostart-button.ts` | Toggle autostart & persist to DB | Native: Custom UI toggles setting player `autoPlay` |
| `trackActivity` | `track-activity.ts` | Tracks play time, saves resume time, increments play count | Native: Custom React hook listening to `currentTime` state |
| `vrMenu` | `vrmode.ts` | 360° VR mode toggle (tag-driven) | Native: WebXR custom controls / 360 viewer plugin |
| `mediaSession` | `media-session.ts` | Media Notification API (artist / cover) | Native: React hook utilizing browser's MediaSession API |
| `wakeSentinel` | `wake-sentinel.ts` | Prevents screen sleep during playback | Native: React hook calling navigator Wake Lock API |
| `chromecast` | `ScenePlayer.tsx:49` | @silvermine/videojs-chromecast | Native: Remote Playback feature integrations |
| `airPlay` | `ScenePlayer.tsx:47` | @silvermine/videojs-airplay | Native: Remote Playback feature integrations |
| `mobileUi` | `ScenePlayer.tsx:610` | videojs-mobile-ui (lock-to-landscape) | Native: Responsive CSS layouts / React hooks |
| `abLoopPlugin` | `ScenePlayer.tsx:50` | videojs-abloop A-B loop region | Native: Custom React hook tracking and resetting `currentTime` |
| `livePlugin` | `live.ts` | "Live" stream indicator | Native: `Live` feature state indicator |

---

## 2. VIDEO STREAMING & TRANSCODING (Backend — Go + FFmpeg)

### 2.1 Streaming Endpoints
```
GET /scene/{id}/stream          → StreamDirect  (original file, HTTP range)
GET /scene/{id}/stream.mp4      → StreamMp4     (fragmented MP4 transcode)
GET /scene/{id}/stream.webm     → StreamWebM    (VP9/Opus transcode)
GET /scene/{id}/stream.mkv      → StreamMKV     (MKV copy-through)
GET /scene/{id}/stream.m3u8     → StreamHLS     (HLS manifest)
GET /scene/{id}/stream.m3u8/{n}.ts → StreamHLSSegment
GET /scene/{id}/stream.mpd      → StreamDASH    (DASH manifest)
GET /scene/{id}/stream.mpd/{n}_v.webm → StreamDASHVideoSegment
GET /scene/{id}/stream.mpd/{n}_a.webm → StreamDASHAudioSegment
```
Source: `routes_scene.go:55–91`

---

## 2.2 HLS Segmented Streaming — Smart Seek Logic
| Feature | File | Detail |
|---------|------|--------|
| 2-second segments | `stream_segmented.go:33` | `segmentLength = 2` |
| Max segment buffer ahead | `stream_segmented.go:44` | `maxSegmentBuffer = 15` |
| Seek detection gap | `stream_segmented.go:40` | `maxSegmentGap = 5` |
| Restart transcode on seek beyond gap | `stream_segmented.go:786–902` | `checkTranscode()` monitors waiting segments |
| Max idle before cleanup | `stream_segmented.go:48` | `maxIdleTime = 30s` |
| Per-segment temp file (`.N.ts`) | `stream_segmented.go:362–401` | `checkSegments()` renames `.N.ts` → `N.ts` |
| Concurrent waiting segment channels | `stream_segmented.go:275–283` | `waitingSegment.available chan error` |
| Monitor interval | `stream_segmented.go:36` | `monitorInterval = 200ms` |
| Max segment wait | `stream_segmented.go:35` | `maxSegmentWait = 15s` |

```go
// Detect seek: if requested segment is more than 5 ahead → restart transcode
if segment > stream.lastSegment + maxSegmentGap {
    sm.stopTranscode(stream)
    sm.startTranscode(stream, segment, done)
}
```

---

### 2.3 DASH Streaming
| Feature | File | Detail |
|---------|------|--------|
| DASH manifest (MPD) generation | `stream_segmented.go:484–586` | `mpd.NewMPD()` → VP9 video + Opus audio |
| Separate video/audio WebM segments | `stream_segmented.go:128–184` | `StreamTypeDASHVideo` / `StreamTypeDASHAudio` |
| Frame-rate fraction detection | `stream_segmented.go:510–528` | handles 23.976 (× 1.001/1000) |
| Resolution capping in manifest | `stream_segmented.go:538–554` | scale factor applied to reported width/height |

---

### 2.4 Transcode Pipeline — Full Logic
| Feature | File | Detail |
|---------|------|--------|
| Seek before transcode start | `stream_transcode.go:204–206` | `-ss {startTime}` ffmpeg arg |
| Hardware codec auto-selection | `stream_transcode.go:152–184` | `FileGetCodec()` checks hw support |
| Codec copy when possible | `stream_transcode.go:165–175` | copy if H.264 file → MP4, VP8/9 → WebM |
| `frag_keyframe+empty_moov` | `stream_transcode.go:100` | enables HTTP streaming for MP4 |
| Output piped to HTTP response | `stream_transcode.go:218` | `args.Output("pipe:")` |
| Stderr drained to avoid deadlock | `stream_transcode.go:270–288` | goroutine `io.ReadAll(stderr)` |
| Context-based kill on disconnect | `stream_transcode.go:224–225` | `NewStreamRequestContext(w, r)` |

---

### 2.5 Hardware Acceleration
| Codec | Backend | Platforms |
|-------|---------|-----------|
| `h264_nvenc` (N264 / N264H) | NVIDIA CUDA | Linux/Windows |
| `h264_qsv` (I264 / I264C) | Intel QSV | Linux/Windows |
| `h264_amf` (A264) | AMD AMF | Windows |
| `h264_videotoolbox` (M264) | Apple VideoToolbox | macOS |
| `h264_vaapi` (V264) | VAAPI | Linux |
| `h264_v4l2m2m` (R264) | V4L2 | Linux ARM |
| `h264_omx` (O264) | OMX | Raspberry Pi |
| `h264_rkmpp` (RK264) | Rockchip MPP | Rockchip SoC |
| `vp9_qsv` (IVP9) | Intel QSV | Linux/Windows |
| `vp9_vaapi` (VVP9) | VAAPI VP9 | Linux |

Source: `codec_hardware.go:18–33`  

**Test at startup:** `InitHWSupport()` runs a 0.1s null encode to probe each codec.  
**Full-hw path:** `hwCanFullHWTranscode()` tests actual file → chooses hw upload vs. full decode-on-GPU.

---

## 3. VIDEO FILE INFO (FFprobe Metadata)

### 3.1 Extracted Fields
```go
type VideoFile struct {
    Path                string
    Title, Comment      string
    Container           string     // e.g. "mov,mp4,m4a,3gp..."
    FileDuration        float64    // declared duration (seconds)
    VideoStreamDuration float64    // stream duration (can differ)
    StartTime           float64
    Bitrate             int64
    Size                int64
    CreationTime        time.Time

    VideoCodec   string   // "h264", "vp9", etc.
    VideoBitrate int64
    Width, Height int
    FrameRate    float64  // rounded to 2 decimals
    Rotation     int64    // 0/90/180/270
    FrameCount   int64

    AudioCodec  string
}
```
Source: `ffprobe.go:87–116`

### 3.2 Key Functions
| Function | File | What it does |
|----------|------|--------------|
| `NewVideoFile(path)` | `ffprobe.go:216` | Run ffprobe JSON, return `VideoFile` |
| `GetReadFrameCount(path)` | `ffprobe.go:250` | `-count_frames` accurate frame count |
| `parse(path, probeJSON)` | `ffprobe.go:267` | Map JSON → struct; handle rotation |
| `TranscodeScale(maxSize)` | `ffprobe.go:121` | Compute scale dims maintaining AR |
| `isRotated(stream)` | `ffprobe.go:349` | Check Tags.Rotate + SideDataList |
| `getAudioStream()` / `getVideoStream()` | `ffprobe.go:368–382` | Skip attached pics; prefer default stream |
| `ValidateFFProbe(path)` | `ffprobe.go:23` | Run `-h` to verify binary |
| `ResolveFFProbe(path, fallback)` | `ffprobe.go:68` | Resolve path → env → fallback |

---

## 4. CONTENT GENERATION TASKS

### 4.1 Screenshot / Cover
- `GET /scene/{id}/screenshot` — serve stored screenshot or default placeholder  
- Task: `task_generate_screenshot.go`  
- Source: `routes_scene.go:246–260`

### 4.2 Preview Clip + Animated WebP
- `GET /scene/{id}/preview` — serve `.mp4` preview clip (short loop)  
- `GET /scene/{id}/webp` — serve animated `.webp` preview  
- Task: `task_generate_preview.go`

### 4.3 Sprite Sheet + VTT (Seek Thumbnails)
- `GET /vtt/thumbs` — serve `*_thumbs.vtt` mapping timestamps → sprite coordinates  
- `GET /vtt/sprite` — serve `*_sprite.jpg` sprite sheet  
- Task: `generator_sprite.go` (generates N frames as JPEG strip + VTT)

### 4.4 Scene Markers
- `GET /scene/{id}/scene_marker/{id}/stream` — mp4 clip at marker timestamp  
- `GET /scene/{id}/scene_marker/{id}/preview` — WebP preview at marker  
- `GET /scene/{id}/scene_marker/{id}/screenshot` — JPEG at marker  
- Task: `task_generate_markers.go`

### 4.5 Perceptual Hash (pHash)
- Task: `task_generate_phash.go` — `phash` fingerprint for duplicate detection  
- Scene duplicate checker: `SceneDuplicateChecker/` component

### 4.6 Interactive Heatmap
- `GET /scene/{id}/interactive_heatmap` — colour heatmap PNG from funscript  
- Task: `task_generate_interactive_heatmap_speed.go` + `generator_interactive_heatmap_speed.go`

### 4.7 Clip Preview (GIF-like)
- Task: `task_generate_clip_preview.go`

### 4.8 Image pHash
- Task: `task_generate_image_phash.go`

### 4.9 Image Thumbnail
- Task: `task_generate_image_thumbnail.go`

---

## 5. LIBRARY & SCENE MANAGEMENT

### 5.1 Scanning
- `task_scan.go` — recursive FS scan, file hashing (OSHash + MD5), fingerprint storage  
- `.stashignore` support: `scan_stashignore_test.go`  
- Exclude patterns: `exclude_files.go`

### 5.2 Scene Metadata
GraphQL mutations: `resolver_mutation_scene.go` (36 KB)  
- Create, Update, Destroy scene  
- Add/remove tags, performers, studio, group  
- Set title, details, date, URL, rating  
- Save activity (resume time, play count, play duration)

### 5.3 File Name Parser
- `SceneFilenameParser/` — parse structured filenames into metadata fields

### 5.4 Auto-tagging
- `task_autotag.go` — match performers/studios/tags by filename substring

### 5.5 Identify
- `task_identify.go` — scrape metadata from StashDB / custom scrapers and apply

### 5.6 Import / Export
- `task_import.go` / `task_export.go` — JSON-based import/export of entire library

### 5.7 Tagger
- `Tagger/` component — manual "tagger" UI to bulk match scenes to scrapers

### 5.8 Filename-based Hash Modes
- `task_migrate_hash.go` — switch between MD5 and OSHash naming algorithms

---

## 6. SYSTEM FEATURES

### 6.1 Authentication
- `authentication.go` — session-based auth, API key support  
- `apikey.go` — generate/revoke API keys  
- `session.go` — cookie-based session management

### 6.2 Job Queue
- `pkg/job/` — generic background job queue  
- GraphQL subscriptions for job progress: `resolver_subscription_job.go`

### 6.3 Plugin System
- `pkg/plugin/` — JS / Python plugin runners  
- Plugin API exposed via `pluginApi.tsx` (59 KB type definitions)

### 6.4 Scraper Framework
- `pkg/scraper/` — generic scraper engine (XPath, JSON, CDP)  
- StashBox integration: `pkg/stashbox/`

### 6.5 DLNA Server
- `internal/dlna/` — UPnP/DLNA media server  
- Mutations: `resolver_mutation_dlna.go`

### 6.6 Configuration
- `resolver_mutation_configure.go` (22 KB) — all runtime settings  
- `manager/config/` — typed config struct

### 6.7 Package Manager
- `resolver_mutation_package.go` — install/update/remove scraper/plugin packages

### 6.8 Backup & Restore
- `backup.go` — SQLite DB backup logic

### 6.9 Running Streams Registry
- `running_streams.go` — global map `dir → runningStream`; idle-stream cleanup

---

## 7. PARITY GAP SUMMARY (for Rust Media Manager)

| Stash Feature | Media Manager Status | Priority | Notes on Video.js v10 Parity |
|---------------|---------------------|----------|------------------------------|
| HLS segmented streaming with seek-restart | ✅ Implemented (Parity) | 🔴 High | Backend parity achieved. |
| DASH segmented streaming | ✅ Implemented (Parity) | 🔴 High | Backend parity achieved. |
| Hardware codec auto-detection (NVENC/QSV/VAAPI/Rockchip) | ⚠️ Partial | 🔴 High | Backend transcoding optimization. |
| Sprite VTT thumbnails for scrubber | ❌ Missing | 🟡 Medium | Natively supported via v10 `<Thumbnail>` component. |
| Scene markers on progress bar | ❌ Missing | 🟡 Medium | Custom overlay divs on `<TimeSlider>` using React. |
| A-B loop region | ❌ Missing | 🟢 Low | Implement via custom React hook tracking time ranges. |
| Multi-source fallback (auto next on error) | ❌ Missing | 🔴 High | Handle via v10 `Error` store state hook. |
| Caption/subtitle multi-language | ❌ Missing | 🟡 Medium | Natively supported via v10 `<track>` and `<CaptionsButton>`. |
| Resume time persistence | ❌ Missing | 🟡 Medium | Implement via React hook syncing to API on pause/unload. |
| Play count / activity tracking | ❌ Missing | 🟡 Medium | Hook into `currentTime` updates to report activity. |
| Funscript / interactive haptic sync | ❌ Missing | 🟢 Low | Subscribe to player state in React to trigger API. |
| VR mode toggle | ❌ Missing | 🟢 Low | Implement custom WebXR player controls if needed. |
| Chromecast / AirPlay | ❌ Missing | 🟢 Low | Use v10 Remote Playback state. |
| Wake lock during playback | ❌ Missing | 🟢 Low | Handled using standard Screen Wake Lock API in React hook. |
| MediaSession OS notifications | ❌ Missing | 🟢 Low | Implement standard browser navigator hook. |
| pHash duplicate detection | ❌ Missing | 🟡 Medium | Backend feature. |
| DLNA server | ❌ Missing | 🟢 Low | Backend feature. |
| Plugin system | ❌ Missing | 🟢 Low | Core app architecture. |
