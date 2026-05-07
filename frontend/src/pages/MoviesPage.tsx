import React, { useMemo, useState } from 'react';
import { Film } from 'lucide-react';
import Hero from '../components/Hero';
import Sidebar from '../components/Sidebar';
import MediaGrid from '../components/MediaGrid';
import { api } from '../api/adapter';

interface MoviesPageProps {
  movies: any[];
  libraries: any[];
  selectedLibrary: number | null;
  setSelectedLibrary: (id: number | null) => void;
  tvShowsCount: number;
  searchQuery: string;
  onItemClick: (item: any, e: React.MouseEvent) => void;
  onPlayClick: (item: any, e: React.MouseEvent) => void;
  selectedIds: number[];
  selectionMode: boolean;
  setSelectionMode: (mode: boolean) => void;
  setSelectedIds: (ids: number[] | ((prev: number[]) => number[])) => void;
  genreFilter: string;
  setGenreFilter: (genre: string) => void;
  languageFilter: string;
  setLanguageFilter: (lang: string) => void;
  allGenres: string[];
  allLanguages: string[];
  showFilterMenu: boolean;
  setShowFilterMenu: (show: boolean) => void;
}

const MoviesPage: React.FC<MoviesPageProps> = (props) => {
  const filteredMovies = useMemo(() => {
    return props.movies.filter(m => 
      m.title.toLowerCase().includes(props.searchQuery.toLowerCase()) ||
      (m.genres && m.genres.toLowerCase().includes(props.searchQuery.toLowerCase()))
    );
  }, [props.movies, props.searchQuery]);

  const heroItem = useMemo(() => {
    return props.movies.find(m => m.backdrop_url && m.status === 'matched') || props.movies[0];
  }, [props.movies]);

  return (
    <div className="pb-24">
      <Hero 
        item={heroItem} 
        onPlay={(item) => api.playMovie(item.id)} 
        onDetails={(item) => props.onItemClick(item, {} as any)} 
      />
      
      <div className={`flex flex-col md:flex-row gap-8 px-4 md:px-12 ${heroItem ? '-mt-20 relative z-10' : 'pt-24'}`}>
        <Sidebar 
          libraries={props.libraries}
          selectedLibrary={props.selectedLibrary}
          setSelectedLibrary={props.setSelectedLibrary}
          moviesCount={props.movies.length}
          tvShowsCount={props.tvShowsCount}
        />
        
        <div className="flex-1 min-w-0">
          <MediaGrid 
            title="My Collection"
            icon={<Film className="w-6 h-6 text-red-600" />}
            items={filteredMovies}
            itemCount={filteredMovies.length}
            onItemClick={props.onItemClick}
            onPlayClick={props.onPlayClick}
            selectedIds={props.selectedIds}
            selectionMode={props.selectionMode}
            setSelectionMode={props.setSelectionMode}
            setSelectedIds={props.setSelectedIds}
            selectedLibrary={props.selectedLibrary}
            genreFilter={props.genreFilter}
            setGenreFilter={props.setGenreFilter}
            languageFilter={props.languageFilter}
            setLanguageFilter={props.setLanguageFilter}
            allGenres={props.allGenres}
            allLanguages={props.allLanguages}
            showFilterMenu={props.showFilterMenu}
            setShowFilterMenu={props.setShowFilterMenu}
          />
        </div>
      </div>
    </div>
  );
};

export default MoviesPage;
