import re
import sys

def fix_file(path):
    with open(path, 'r', encoding='utf-8') as f:
        content = f.read()

    # delete_library(&state.pool, id) -> media_core::models::LibraryId(id)
    content = re.sub(r'delete_library\(([^,]+),\s*id\)', r'delete_library(\1, media_core::models::LibraryId(id))', content)

    # get_all_movies(&state.pool, library_id, genre, language)
    content = re.sub(r'get_all_movies\(([^,]+),\s*library_id,\s*genre,\s*language\)', r'get_all_movies(\1, library_id.map(media_core::models::LibraryId), genre, language)', content)
    
    # get_all_tv_shows(&state.pool, library_id, genre, language)
    content = re.sub(r'get_all_tv_shows\(([^,]+),\s*library_id,\s*genre,\s*language\)', r'get_all_tv_shows(\1, library_id.map(media_core::models::LibraryId), genre, language)', content)

    # get_seasons_by_show_id(&state.pool, show_id) -> media_core::models::TvShowId(show_id)
    content = re.sub(r'get_seasons_by_show_id\(([^,]+),\s*show_id\)', r'get_seasons_by_show_id(\1, media_core::models::TvShowId(show_id))', content)

    # get_episodes_by_season_id(&state.pool, season_id) -> media_core::models::SeasonId(season_id)
    content = re.sub(r'get_episodes_by_season_id\(([^,]+),\s*season_id\)', r'get_episodes_by_season_id(\1, media_core::models::SeasonId(season_id))', content)

    # l.id == library_id -> l.id == media_core::models::LibraryId(library_id)
    content = re.sub(r'l\.id == library_id\)', r'l.id == media_core::models::LibraryId(library_id))', content)
    
    # l.id == id) -> l.id == media_core::models::LibraryId(id))
    content = re.sub(r'l\.id == id\)', r'l.id == media_core::models::LibraryId(id))', content)
    
    # s.id == id) -> s.id == media_core::models::TvShowId(id))
    content = re.sub(r's\.id == id\)', r's.id == media_core::models::TvShowId(id))', content)

    # get_movie_by_id(&pool, id) -> media_core::models::MovieId(id)
    content = re.sub(r'get_movie_by_id\(([^,]+),\s*id\)', r'get_movie_by_id(\1, media_core::models::MovieId(id))', content)
    
    # get_tv_show_by_id(&pool, id) -> media_core::models::TvShowId(id)
    content = re.sub(r'get_tv_show_by_id\(([^,]+),\s*id\)', r'get_tv_show_by_id(\1, media_core::models::TvShowId(id))', content)
    
    # update_movie(&state.pool, id,
    content = re.sub(r'update_movie\(([^,]+),\s*id,\s*&title', r'update_movie(\1, media_core::models::MovieId(id), &title', content)
    
    # update_tv_show(&state.pool, id,
    content = re.sub(r'update_tv_show\(([^,]+),\s*id,\s*&title', r'update_tv_show(\1, media_core::models::TvShowId(id), &title', content)

    # scrape_movie(id, -> scrape_movie(id.into(),
    content = re.sub(r'scrape_movie\(([^,]+),\s*&title_clone', r'scrape_movie(\1.into(), &title_clone', content)
    content = re.sub(r'scrape_movie\(movie\.id,\s*&movie\.title', r'scrape_movie(movie.id.0, &movie.title', content)
    
    # scrape_tv_show(id, -> scrape_tv_show(id.into(),
    content = re.sub(r'scrape_tv_show\(([^,]+),\s*&title_clone', r'scrape_tv_show(\1.into(), &title_clone', content)
    content = re.sub(r'scrape_tv_show\(show\.id,\s*&show\.title', r'scrape_tv_show(show.id.0, &show.title', content)

    # Some(id) -> Some(media_core::models::LibraryId(id))
    content = re.sub(r'Some\(id\)', r'Some(media_core::models::LibraryId(id))', content)
    
    # get_movies_by_ids(&pool, &ids) where ids is Vec<i64>
    # We should change ids: Vec<i64> to ids: Vec<MovieId> where it's parsed.
    # We'll just manually replace &ids with &ids.iter().map(|&id| media_core::models::MovieId(id)).collect::<Vec<_>>()
    content = re.sub(r'get_movies_by_ids\(([^,]+),\s*&ids\)', r'get_movies_by_ids(\1, &ids.iter().map(|&x| media_core::models::MovieId(x)).collect::<Vec<_>>())', content)
    content = re.sub(r'get_tv_shows_by_ids\(([^,]+),\s*&ids\)', r'get_tv_shows_by_ids(\1, &ids.iter().map(|&x| media_core::models::TvShowId(x)).collect::<Vec<_>>())', content)

    # get_movies_by_ids(&pool_clone, &all_ids_movies) ->
    content = re.sub(r'get_movies_by_ids\(([^,]+),\s*&all_ids_movies\)', r'get_movies_by_ids(\1, &all_ids_movies)', content)
    
    # map(|m| (m.id, m.title, m.year, "movie")) -> map(|m| (m.id.0, m.title, m.year, "movie"))
    content = content.replace('map(|m| (m.id, m.title, m.year, "movie"))', 'map(|m| (m.id.0, m.title, m.year, "movie"))')
    content = content.replace('map(|s| (s.id, s.title, None, "tv"))', 'map(|s| (s.id.0, s.title, None, "tv"))')

    # Ok(id) -> Ok(id.into())
    content = content.replace('Ok(id)\n', 'Ok(id.into())\n')

    # Vec<i64> = movies.into_iter().filter(...).map(|m| m.id.0).collect()
    content = content.replace('.map(|m| m.id)', '.map(|m| m.id.0)')
    content = content.replace('.map(|s| s.id)', '.map(|s| s.id.0)')

    # get_tv_shows_by_ids(&pool_clone, &all_ids_tv)
    content = re.sub(r'get_tv_shows_by_ids\(([^,]+),\s*&all_ids_tv\)', r'get_tv_shows_by_ids(\1, &all_ids_tv.iter().map(|&x| media_core::models::TvShowId(x)).collect::<Vec<_>>())', content)
    content = re.sub(r'get_movies_by_ids\(([^,]+),\s*&all_ids_movies\)', r'get_movies_by_ids(\1, &all_ids_movies.iter().map(|&x| media_core::models::MovieId(x)).collect::<Vec<_>>())', content)

    with open(path, 'w', encoding='utf-8') as f:
        f.write(content)

fix_file('apps/server/src/main.rs')
fix_file('apps/desktop/src/main.rs')
fix_file('apps/cli/src/main.rs')
