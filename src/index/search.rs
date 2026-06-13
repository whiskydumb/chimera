use std::collections::{HashMap, HashSet};

use anyhow::Result;
use globset::Glob;
use nucleo_matcher::pattern::{CaseMatching, Normalization, Pattern};
use nucleo_matcher::{Config, Matcher, Utf32Str};
use rusqlite::Connection;

use crate::index::db::{self, Record};

/// a single search hit with its display record and ranking score.
#[derive(Debug, Clone)]
pub struct Hit {
    pub record: Record,
    pub score: i64,
}

/// results split into a "names" section (name/path/tags) and a "content" section
/// (file body). callers render them as separate UI sections.
#[derive(Debug, Default)]
pub struct Results {
    pub names: Vec<Hit>,
    pub content: Vec<Hit>,
}

/// runs a search. an empty query browses everything; a glob (`*.rs`) filters
/// names; otherwise names are fuzzy-matched (supporting `'exact ^prefix suffix$
/// !not`) and content is full-text matched, content de-duplicated against names.
pub fn search(conn: &Connection, query: &str, limit: usize) -> Result<Results> {
    let query = query.trim();
    let records = db::all(conn)?;

    if query.is_empty() {
        let mut names: Vec<Hit> = records
            .into_iter()
            .map(|record| Hit { record, score: 0 })
            .collect();
        names.sort_by(|a, b| a.record.rel_path.cmp(&b.record.rel_path));
        names.truncate(limit);
        return Ok(Results {
            names,
            content: Vec::new(),
        });
    }

    if is_glob(query) {
        let names = glob_search(query, &records, limit);
        return Ok(Results {
            names,
            content: Vec::new(),
        });
    }

    let by_id: HashMap<String, Record> =
        records.iter().cloned().map(|r| (r.id.clone(), r)).collect();

    let mut names = fuzzy(query, &records);
    names.truncate(limit);
    let name_ids: HashSet<&str> = names.iter().map(|hit| hit.record.id.as_str()).collect();

    // content: full-text matches not already shown under names.
    let mut content = Vec::new();
    if let Some(match_query) = fts_query(query)
        && let Ok(ids) = db::search_fts(conn, &match_query, limit * 4)
    {
        for id in ids {
            if name_ids.contains(id.as_str()) {
                continue;
            }
            if let Some(record) = by_id.get(&id) {
                content.push(Hit {
                    record: record.clone(),
                    score: 0,
                });
            }
            if content.len() >= limit {
                break;
            }
        }
    }

    Ok(Results { names, content })
}

/// the query's positive literal terms (operators stripped, inverse/glob dropped),
/// reused for fts and for highlighting matches in the preview.
pub fn literal_terms(query: &str) -> Vec<String> {
    query
        .split_whitespace()
        .filter(|tok| !tok.starts_with('!') && !tok.contains(['*', '?', '[']))
        .map(|tok| tok.trim_start_matches(['\'', '^']).trim_end_matches('$'))
        .filter(|tok| !tok.is_empty())
        .map(str::to_string)
        .collect()
}

fn is_glob(query: &str) -> bool {
    query.contains('*') || query.contains('?') || query.contains('[')
}

fn glob_search(query: &str, records: &[Record], limit: usize) -> Vec<Hit> {
    let matcher = match Glob::new(query) {
        Ok(glob) => glob.compile_matcher(),
        Err(_) => return Vec::new(),
    };
    let mut hits: Vec<Hit> = records
        .iter()
        .filter(|r| matcher.is_match(&r.name) || matcher.is_match(&r.rel_path))
        .map(|r| Hit {
            record: r.clone(),
            score: 0,
        })
        .collect();
    hits.sort_by(|a, b| a.record.rel_path.cmp(&b.record.rel_path));
    hits.truncate(limit);
    hits
}

/// fuzzy-matches name/path/tags with nucleo, ranked by score (highest first).
/// nucleo's gap penalties already sink scattered matches, so we rank, never hide.
fn fuzzy(query: &str, records: &[Record]) -> Vec<Hit> {
    let mut matcher = Matcher::new(Config::DEFAULT.match_paths());
    let pattern = Pattern::parse(query, CaseMatching::Smart, Normalization::Smart);
    let mut buf = Vec::new();
    let mut hits = Vec::new();
    for record in records {
        let tags = record.tags.join(" ");
        let mut best: Option<u32> = None;
        for field in [
            record.name.as_str(),
            record.rel_path.as_str(),
            tags.as_str(),
        ] {
            if let Some(score) = pattern.score(Utf32Str::new(field, &mut buf), &mut matcher) {
                best = Some(best.map_or(score, |b| b.max(score)));
            }
        }
        if let Some(score) = best {
            hits.push(Hit {
                record: record.clone(),
                score: score as i64,
            });
        }
    }
    hits.sort_by(|a, b| {
        b.score
            .cmp(&a.score)
            .then_with(|| a.record.rel_path.cmp(&b.record.rel_path))
    });
    hits
}

/// builds an fts5 MATCH expression from the query's literal terms (quoted prefix
/// terms joined by implicit AND). returns `None` when there is nothing to match.
fn fts_query(query: &str) -> Option<String> {
    let parts: Vec<String> = literal_terms(query)
        .into_iter()
        .map(|term| format!("\"{}\"*", term.replace('"', "\"\"")))
        .collect();
    if parts.is_empty() {
        None
    } else {
        Some(parts.join(" "))
    }
}
