// core/src/exporter/mod.rs
use crate::models::{Movie, TVShow};
use anyhow::Result;
use serde_json;
use std::fmt::Write;

pub struct Exporter;

impl Exporter {
    pub fn to_csv(movies: &[Movie], tv_shows: &[TVShow]) -> String {
        let mut csv = String::from("Type,ID,Title,Year,Status,IMDB ID,TMDB ID,Rating\n");
        for m in movies {
            let line = format!(
                "Movie,{},\"{}\",{},{},{},{},{}\n",
                m.id,
                m.title,
                m.year.map(|y| y.to_string()).unwrap_or_default(),
                m.status,
                m.imdb_id.clone().unwrap_or_default(),
                m.tmdb_id.map(|id| id.to_string()).unwrap_or_default(),
                m.rating.map(|r| r.to_string()).unwrap_or_default()
            );
            csv.push_str(&line);
        }
        for t in tv_shows {
            let line = format!(
                "TV Show,{},\"{}\",{},{},{},{},{}\n",
                t.id,
                t.title,
                "", // TV Shows don't have a year field
                t.status,
                t.imdb_id.clone().unwrap_or_default(),
                t.tmdb_id.map(|id| id.to_string()).unwrap_or_default(),
                t.rating.map(|r| r.to_string()).unwrap_or_default()
            );
            csv.push_str(&line);
        }
        csv
    }

    pub fn to_json(movies: &[Movie], tv_shows: &[TVShow]) -> Result<String> {
        #[derive(serde::Serialize)]
        struct ExportData<'a> {
            movies: &'a [Movie],
            tv_shows: &'a [TVShow],
        }
        let data = ExportData { movies, tv_shows };
        Ok(serde_json::to_string_pretty(&data)?)
    }

    pub fn to_html(movies: &[Movie], tv_shows: &[TVShow]) -> String {
        let mut html = String::from(r#"<!DOCTYPE html>
<html>
<head>
    <meta charset="UTF-8">
    <title>Library Export</title>
    <style>
        body { font-family: sans-serif; background: #1a1a2e; color: #e0e0e0; padding: 20px; }
        table { width: 100%; border-collapse: collapse; background: #16213e; margin-bottom: 20px; }
        th, td { padding: 12px; text-align: left; border-bottom: 1px solid #0f3460; }
        th { background: #6c63ff; color: white; }
        .matched { color: #4caf50; }
        .unmatched { color: #f44336; }
    </style>
</head>
<body>
    <h1>🎬 Media Manager Library</h1>
    <h2>Movies</h2>
    <table>
        <thead>
            <tr><th>Title</th><th>Year</th><th>Rating</th><th>Status</th></tr>
        </thead>
        <tbody>
"#);

        for m in movies {
            let status_cls = if m.status == crate::models::MediaStatus::Matched { "matched" } else { "unmatched" };
            let _ = write!(
                html,
                r#"<tr><td><strong>{}</strong></td><td>{}</td><td>{}</td><td><span class="{}">{}</span></td></tr>"#,
                m.title,
                m.year.map(|y| y.to_string()).unwrap_or_else(|| "—".to_string()),
                m.rating.map(|r| r.to_string()).unwrap_or_else(|| "—".to_string()),
                status_cls,
                m.status
            );
        }

        html.push_str("</tbody></table>\n<h2>TV Shows</h2>\n<table><thead><tr><th>Title</th><th>Year</th><th>Rating</th><th>Status</th></tr></thead><tbody>\n");

        for t in tv_shows {
            let status_cls = if t.status == crate::models::MediaStatus::Matched { "matched" } else { "unmatched" };
            let _ = write!(
                html,
                r#"<tr><td><strong>{}</strong></td><td>{}</td><td>{}</td><td><span class="{}">{}</span></td></tr>"#,
                t.title,
                "—".to_string(), // TV Shows don't have a year field
                t.rating.map(|r| r.to_string()).unwrap_or_else(|| "—".to_string()),
                status_cls,
                t.status
            );
        }

        html.push_str("</tbody></table></body></html>");
        html
    }

    /// Export movies and tv shows to XLSX (Excel) format
    pub fn to_xlsx(movies: &[Movie], tv_shows: &[TVShow]) -> Result<Vec<u8>> {
        use rust_xlsxwriter::*;

        let mut workbook = Workbook::new();
        let worksheet = workbook.add_worksheet();

        // Header formatting
        let header_format = Format::new()
            .set_bold()
            .set_font_color(Color::White)
            .set_background_color(Color::RGB(0x6C63FF))
            .set_font_size(12);

        let data_format = Format::new()
            .set_font_size(11);

        let rating_format = Format::new()
            .set_font_size(11)
            .set_num_format("0.0");

        // Write headers
        let headers = ["Type", "ID", "Title", "Year", "Status", "IMDB ID", "TMDB ID", "Rating", "Genres", "Language"];
        for (col, header) in headers.iter().enumerate() {
            worksheet.write_string_with_format(0, col as u16, *header, &header_format)?;
        }

        // Set column widths
        worksheet.set_column_width(0, 10)?;  // Type
        worksheet.set_column_width(1, 8)?;   // ID
        worksheet.set_column_width(2, 40)?;  // Title
        worksheet.set_column_width(3, 8)?;   // Year
        worksheet.set_column_width(4, 12)?;  // Status
        worksheet.set_column_width(5, 14)?;  // IMDB ID
        worksheet.set_column_width(6, 10)?;  // TMDB ID
        worksheet.set_column_width(7, 8)?;   // Rating
        worksheet.set_column_width(8, 30)?;  // Genres
        worksheet.set_column_width(9, 12)?;  // Language

        let mut current_row = 1;

        // Write data rows
        for m in movies.iter() {
            worksheet.write_string_with_format(current_row, 0, "Movie", &data_format)?;
            worksheet.write_number_with_format(current_row, 1, m.id.0 as f64, &data_format)?;
            worksheet.write_string_with_format(current_row, 2, &m.title, &data_format)?;
            
            if let Some(year) = m.year {
                worksheet.write_number_with_format(current_row, 3, year as f64, &data_format)?;
            }
            
            worksheet.write_string_with_format(current_row, 4, &format!("{}", m.status), &data_format)?;
            
            if let Some(ref imdb) = m.imdb_id {
                worksheet.write_string_with_format(current_row, 5, imdb, &data_format)?;
            }
            
            if let Some(tmdb) = m.tmdb_id {
                worksheet.write_number_with_format(current_row, 6, tmdb as f64, &data_format)?;
            }
            
            if let Some(rating) = m.rating {
                worksheet.write_number_with_format(current_row, 7, rating as f64, &rating_format)?;
            }
            
            if let Some(ref genres) = m.genres {
                let genre_list: Vec<String> = serde_json::from_str(genres).unwrap_or_default();
                worksheet.write_string_with_format(current_row, 8, &genre_list.join(", "), &data_format)?;
            }
            
            if let Some(ref lang) = m.language {
                worksheet.write_string_with_format(current_row, 9, lang, &data_format)?;
            }
            current_row += 1;
        }

        for t in tv_shows.iter() {
            worksheet.write_string_with_format(current_row, 0, "TV Show", &data_format)?;
            worksheet.write_number_with_format(current_row, 1, t.id.0 as f64, &data_format)?;
            worksheet.write_string_with_format(current_row, 2, &t.title, &data_format)?;
            
            worksheet.write_string_with_format(current_row, 3, "", &data_format)?;
            
            worksheet.write_string_with_format(current_row, 4, &format!("{}", t.status), &data_format)?;
            
            if let Some(ref imdb) = t.imdb_id {
                worksheet.write_string_with_format(current_row, 5, imdb, &data_format)?;
            }
            
            if let Some(tmdb) = t.tmdb_id {
                worksheet.write_number_with_format(current_row, 6, tmdb as f64, &data_format)?;
            }
            
            if let Some(rating) = t.rating {
                worksheet.write_number_with_format(current_row, 7, rating as f64, &rating_format)?;
            }
            
            if let Some(ref genres) = t.genres {
                let genre_list: Vec<String> = serde_json::from_str(genres).unwrap_or_default();
                worksheet.write_string_with_format(current_row, 8, &genre_list.join(", "), &data_format)?;
            }
            
            if let Some(ref lang) = t.language {
                worksheet.write_string_with_format(current_row, 9, lang, &data_format)?;
            }
            current_row += 1;
        }

        // Enable auto-filter for the header row
        if current_row > 1 {
            worksheet.autofilter(0, 0, current_row - 1, 9)?;
        }

        let buffer = workbook.save_to_buffer()?;
        Ok(buffer)
    }
}
