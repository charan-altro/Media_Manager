import React, { useMemo } from 'react';
import { Tv } from 'lucide-react';
import Hero from '../components/Hero';
import Sidebar from '../components/Sidebar';
import MediaGrid from '../components/MediaGrid';

interface TvShowsPageProps {
  tvShows: any[];
  libraries: any[];
  selectedLibrary: number | null;
  setSelectedLibrary: (id: number | null) => void;
  moviesCount: number;
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

const TvShowsPage: React.FC<TvShowsPageProps> = (props) => {
  const filteredShows = useMemo(() => {
    return props.tvShows.filter(s => 
      s.title.toLowerCase().includes(props.searchQuery.toLowerCase()) ||
      (s.genres && s.genres.toLowerCase().includes(props.searchQuery.toLowerCase()))
    );
  }, [props.tvShows, props.searchQuery]);

  const heroItem = useMemo(() => {
    return props.tvShows.find(s => s.backdrop_url && s.status === 'matched') || props.tvShows[0];
  }, [props.tvShows]);

  return (
    <div className="pb-24">
      <Hero 
        item={heroItem} 
        onPlay={(item) => props.onItemClick(item, {} as any)} 
        onDetails={(item) => props.onItemClick(item, {} as any)} 
      />
      
      <div className={`flex flex-col md:flex-row gap-8 px-4 md:px-12 ${heroItem ? '-mt-20 relative z-10' : 'pt-24'}`}>
        <Sidebar 
          libraries={props.libraries}
          selectedLibrary={props.selectedLibrary}
          setSelectedLibrary={props.setSelectedLibrary}
          moviesCount={props.moviesCount}
          tvShowsCount={props.tvShows.length}
        />
        
        <div className="flex-1 min-w-0">
          <MediaGrid 
            title="Television"
            icon={<Tv className="w-6 h-6 text-red-600" />}
            items={filteredShows}
            itemCount={filteredShows.length}
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

export default React.memo(TvShowsPage);
