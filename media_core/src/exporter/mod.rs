// core/src/exporter/mod.rs
use crate::models::Movie;
use anyhow::Result;
use serde_json;
use std::fmt::Write;

pub struct Exporter;

impl Exporter {
    pub fn to_csv(movies: &[Movie]) -> String {
        let mut csv = String::from("ID,Title,Year,Status,IMDB ID,TMDB ID,Rating\n");
        for m in movies {
            let line = format!(
                "{},\"{}\",{},{},{},{},{}\n",
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
        csv
    }

    pub fn to_json(movies: &[Movie]) -> Result<String> {
        Ok(serde_json::to_string_pretty(movies)?)
    }

    pub fn to_html(movies: &[Movie]) -> String {
        let mut html = String::from(r#"<!DOCTYPE html>
<html>
<head>
    <meta charset="UTF-8">
    <title>Library Export</title>
    <style>
        body { font-family: sans-serif; background: #1a1a2e; color: #e0e0e0; padding: 20px; }
        table { width: 100%; border-collapse: collapse; background: #16213e; }
        th, td { padding: 12px; text-align: left; border-bottom: 1px solid #0f3460; }
        th { background: #6c63ff; color: white; }
        .matched { color: #4caf50; }
        .unmatched { color: #f44336; }
    </style>
</head>
<body>
    <h1>🎬 Media Manager Library</h1>
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

        html.push_str("</tbody></table></body></html>");
        html
    }
}
