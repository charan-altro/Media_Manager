import { useState, useEffect, useCallback } from 'react';
import { api } from '../api/adapter';

export function useLibraryData() {
  const [libraries, setLibraries] = useState<any[]>([]);
  const [movies, setMovies] = useState<any[]>([]);
  const [tvShows, setTvShows] = useState<any[]>([]);
  const [loading, setLoading] = useState(true);
  
  const [selectedLibrary, setSelectedLibrary] = useState<number | null>(null);
  const [genreFilter, setGenreFilter] = useState('');
  const [languageFilter, setLanguageFilter] = useState('');
  
  const [allGenres, setAllGenres] = useState<string[]>([]);
  const [allLanguages, setAllLanguages] = useState<string[]>([]);

  const loadData = useCallback(async () => {
    try {
      const [libs, movs, shows, genres, langs] = await Promise.all([
        api.getLibraries(),
        api.getMovies(selectedLibrary || undefined, genreFilter, languageFilter),
        api.getTvShows(selectedLibrary || undefined, genreFilter, languageFilter),
        api.getGenres(),
        api.getLanguages()
      ]);
      
      setLibraries(libs);
      setMovies(movs);
      setTvShows(shows);
      setAllGenres(genres);
      setAllLanguages(langs);
    } catch (err) {
      console.error('Failed to load data', err);
    } finally {
      setLoading(false);
    }
  }, [selectedLibrary, genreFilter, languageFilter]);

  useEffect(() => {
    loadData();
  }, [loadData]);

  return {
    libraries,
    movies,
    tvShows,
    loading,
    selectedLibrary,
    setSelectedLibrary,
    genreFilter,
    setGenreFilter,
    languageFilter,
    setLanguageFilter,
    allGenres,
    allLanguages,
    loadData
  };
}