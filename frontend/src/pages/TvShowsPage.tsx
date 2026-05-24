import React, { useMemo } from 'react';
import { Tv } from 'lucide-react';
import Hero from '../components/Hero';
import MediaGrid from '../components/MediaGrid';
import { useMediaStore } from '../context/MediaStoreContext';

const TvShowsPage: React.FC = () => {
  const {
    tvShows,
    selectedLibrary,
    searchQuery,
    genreFilter,
    setGenreFilter,
    languageFilter,
    setLanguageFilter,
    allGenres,
    allLanguages,
    showFilterMenu,
    setShowFilterMenu,
    selectedIds,
    setSelectedIds,
    selectionMode,
    setSelectionMode,
    handleItemClick,
    handlePlayClick,
  } = useMediaStore();

  const genreText = (genres: string | string[] | undefined) =>
    Array.isArray(genres) ? genres.join(' ') : genres || '';

  const filteredShows = useMemo(() => {
    return tvShows.filter(s =>
      s.title.toLowerCase().includes(searchQuery.toLowerCase()) ||
      genreText(s.genres).toLowerCase().includes(searchQuery.toLowerCase())
    );
  }, [tvShows, searchQuery]);

  const heroItem = useMemo(() => {
    return tvShows.find(s => s.backdrop_url && s.status === 'matched') || tvShows[0];
  }, [tvShows]);

  return (
    <div className="pb-24 bg-zinc-950">
      <Hero
        item={heroItem}
        onPlay={(item) => handleItemClick(item)}
        onDetails={(item) => handleItemClick(item)}
      />

      <div className={`px-4 md:px-12 w-full ${heroItem ? '-mt-20 relative z-10' : 'pt-24'}`}>
        <MediaGrid
          title="Television"
          icon={<Tv className="w-6 h-6 text-red-600 animate-pulse" />}
          items={filteredShows}
          itemCount={filteredShows.length}
          onItemClick={handleItemClick}
          onPlayClick={handlePlayClick}
          selectedIds={selectedIds}
          selectionMode={selectionMode}
          setSelectionMode={setSelectionMode}
          setSelectedIds={setSelectedIds}
          selectedLibrary={selectedLibrary}
          genreFilter={genreFilter}
          setGenreFilter={setGenreFilter}
          languageFilter={languageFilter}
          setLanguageFilter={setLanguageFilter}
          allGenres={allGenres}
          allLanguages={allLanguages}
          showFilterMenu={showFilterMenu}
          setShowFilterMenu={setShowFilterMenu}
        />
      </div>
    </div>
  );
};

export default React.memo(TvShowsPage);
