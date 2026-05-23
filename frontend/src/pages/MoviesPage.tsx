import React, { useMemo } from 'react';
import { Film } from 'lucide-react';
import Hero from '../components/Hero';
import Sidebar from '../components/Sidebar';
import MediaGrid from '../components/MediaGrid';
import { api } from '../api/adapter';
import { useMediaStore } from '../context/MediaStoreContext';

const MoviesPage: React.FC = () => {
  const {
    movies,
    tvShows,
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

  const filteredMovies = useMemo(() => {
    return movies.filter(m =>
      m.title.toLowerCase().includes(searchQuery.toLowerCase()) ||
      (m.genres && m.genres.toLowerCase().includes(searchQuery.toLowerCase()))
    );
  }, [movies, searchQuery]);

  const heroItem = useMemo(() => {
    return movies.find(m => m.backdrop_url && m.status === 'matched') || movies[0];
  }, [movies]);

  return (
    <div className="pb-24">
      <Hero
        item={heroItem}
        onPlay={(item) => api.playMovie(item.id)}
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
            title="My Collection"
            icon={<Film className="w-6 h-6 text-red-600" />}
            items={filteredMovies}
            itemCount={filteredMovies.length}
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

export default React.memo(MoviesPage);
