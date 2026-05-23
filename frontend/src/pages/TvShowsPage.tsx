import React, { useMemo } from 'react';
import { Tv } from 'lucide-react';
import Hero from '../components/Hero';
import Sidebar from '../components/Sidebar';
import MediaGrid from '../components/MediaGrid';
import { useMediaStore } from '../context/MediaStoreContext';

const TvShowsPage: React.FC = () => {
  const {
    tvShows,
    movies,
    libraries,
    selectedLibrary,
    setSelectedLibrary,
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

  const filteredShows = useMemo(() => {
    return tvShows.filter(s =>
      s.title.toLowerCase().includes(searchQuery.toLowerCase()) ||
      (s.genres && s.genres.toLowerCase().includes(searchQuery.toLowerCase()))
    );
  }, [tvShows, searchQuery]);

  const heroItem = useMemo(() => {
    return tvShows.find(s => s.backdrop_url && s.status === 'matched') || tvShows[0];
  }, [tvShows]);

  return (
    <div className="pb-24">
      <Hero
        item={heroItem}
        onPlay={(item) => handleItemClick(item)}
        onDetails={(item) => handleItemClick(item)}
      />

      <div className={`flex flex-col md:flex-row gap-8 px-4 md:px-12 ${heroItem ? '-mt-20 relative z-10' : 'pt-24'}`}>
        <Sidebar
          libraries={libraries}
          selectedLibrary={selectedLibrary}
          setSelectedLibrary={setSelectedLibrary}
          moviesCount={movies.length}
          tvShowsCount={tvShows.length}
        />

        <div className="flex-1 min-w-0">
          <MediaGrid
            title="Television"
            icon={<Tv className="w-6 h-6 text-red-600" />}
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
    </div>
  );
};

export default React.memo(TvShowsPage);
