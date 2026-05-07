# SelfHost Media Orchestrator: Feature Matrix

This document provides a detailed breakdown of feature support for Movies and TV Shows within the ecosystem, following the standard media management feature set.

| Feature Description | Movies | TV Shows |
|:--- |:---:|:---:|
| **Scan your data sources**<br>Rapid discovery of new/changed content using Rayon concurrency | :heavy_check_mark: | :heavy_check_mark: |
| **Import and export NFO files**<br>Full support for industry-standard Kodi/Jellyfin NFO formats | :heavy_check_mark: | :heavy_check_mark: |
| **Edit media metadata**<br>Manual adjustments for Title, Year, Plot, and Ratings in the UI | :heavy_check_mark: | :heavy_check_mark: |
| **Rename movies, TV shows and episodes**<br>Change folder and file names based on formal naming styles | :heavy_check_mark: | :heavy_check_mark: |
| **Automatic updates**<br>Receive background updates for metadata and ratings | :heavy_check_mark: | :heavy_check_mark: |
| **Export library data**<br>Native export to HTML, CSV, and JSON formats | :heavy_check_mark: | :heavy_check_mark: |
| **Command line interface (CLI)**<br>Batch processing and automation via terminal | :heavy_check_mark: | :heavy_check_mark: |
| **HTTP interface (REST API)**<br>Full Axum-based API for third-party integrations | :heavy_check_mark: | :heavy_check_mark: |
| **Post processing**<br>Automated tasks (like cleanup) triggered after scraping | :heavy_check_mark: | :heavy_check_mark: |
| **Enhanced aspect ratio detection**<br>Deep media analysis via FFmpeg integration | :heavy_check_mark: | :heavy_check_mark: |
| **TheMovieDB (TMDB) scraper**<br>Localized metadata and artwork for global titles | :heavy_check_mark: | :heavy_check_mark: |
| **TheTVDB (TVDB) scraper**<br>Specialized TV show metadata and episode details | :heavy_check_mark: | :heavy_check_mark: |
| **OMDb API scraper**<br>Integration for IMDB ratings and English metadata | :heavy_check_mark: | :heavy_check_mark: |
| **Universal scraper**<br>Intelligent merging of results from multiple APIs | :heavy_check_mark: | :heavy_check_mark: |
| **AniDB / Trakt.tv scrapers**<br>Metadata for anime and social tracking integration | :heavy_check_mark: | :heavy_check_mark: |
| **Online trailers**<br>Automatic discovery and link generation for trailers | :heavy_check_mark: | :heavy_check_mark: |
| **Subtitle download**<br>Automated retrieval of matching .srt files | :heavy_check_mark: | :heavy_check_mark: |
| **FFmpeg integration**<br>High-speed thumbnail generation and stream analysis | :heavy_check_mark: | :heavy_check_mark: |
| **External tools integration**<br>Support for yt-dlp, MKVToolNix, and custom scripts | :heavy_check_mark: | :heavy_check_mark: |
| **HLS Adaptive Streaming**<br>Optional on-the-fly transcoding for universal playback | :heavy_check_mark: | :heavy_check_mark: |

---
*Key: ✅ = Supported | 🛠️ = Roadmap*
