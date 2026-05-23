import { useState, useEffect, useCallback } from 'react';
import { api, type Library, type Movie, type TVShow } from '../api/adapter';

export function useLibraryData() {
  const [libraries, setLibraries] = useState<Library[]>([]);
  const [movies, setMovies] = useState<Movie[]>([]);
  const [tvShows, setTvShows] = useState<TVShow[]>([]);
  const [loading, setLoading] = useState(true);
  
  const [selectedLibrary, setSelectedLibrary] = useState<number | null>(null);
  const [genreFilter, setGenreFilter] = useState('');
  const [languageFilter, setLanguageFilter] = useState('');
  
  const [allGenres, setAllGenres] = useState<string[]>([]);
  const [allLanguages, setAllLanguages] = useState<string[]>([]);

  // Load static resources only once on mount
  useEffect(() => {
    let active = true;
    const loadStatic = async () => {
      try {
        const [libs, genres, langs] = await Promise.all([
          api.getLibraries(),
          api.getGenres(),
          api.getLanguages()
        ]);
        if (active) {
          setLibraries(libs);
          setAllGenres(genres);
          setAllLanguages(langs);
        }
      } catch (err) {
        console.error('Failed to load static library resources', err);
      }
    };
    loadStatic();
    return () => { active = false; };
  }, []);

  // Fetch movies/shows when selections/filters change
  const loadMedia = useCallback(async () => {
    setLoading(true);
    try {
      const [movs, shows] = await Promise.all([
        api.getMovies(selectedLibrary || undefined, genreFilter, languageFilter),
        api.getTvShows(selectedLibrary || undefined, genreFilter, languageFilter),
      ]);
      setMovies(movs);
      setTvShows(shows);
    } catch (err) {
      console.error('Failed to load media items', err);
    } finally {
      setLoading(false);
    }
  }, [selectedLibrary, genreFilter, languageFilter]);

  useEffect(() => {
    loadMedia();
  }, [loadMedia]);

  // Keep loadData function signature for App.tsx manual updates
  const loadData = useCallback(async () => {
    setLoading(true);
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
      console.error('Failed to reload all library data', err);
    } finally {
      setLoading(false);
    }
  }, [selectedLibrary, genreFilter, languageFilter]);

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