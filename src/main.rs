mod cli;
mod config;
mod editor;
mod index;
mod library;
mod reuse;
mod tui;

use std::path::PathBuf;

use anyhow::Result;
use clap::Parser;
use tracing_subscriber::EnvFilter;

use crate::cli::{Cli, Command};
use crate::config::Paths;
use crate::library::store::{self, AddOptions};

fn main() -> Result<()> {
    let cli = Cli::parse();
    let paths = Paths::resolve()?;
    paths.ensure()?;

    // logging targets stderr, which would corrupt the TUI's alternate screen —
    // so it is enabled for cli subcommands only.
    if cli.command.is_some() {
        init_tracing();
    }

    match cli.command {
        Some(Command::Add {
            paths: inputs,
            category,
            tags,
            description,
        }) => cmd_add(
            &paths,
            &inputs,
            AddOptions {
                category,
                tags,
                description,
            },
        ),
        Some(Command::Search { query, limit }) => cmd_search(&paths, &query, limit),
        Some(Command::Reindex) => cmd_reindex(&paths),
        Some(Command::List) => cmd_list(&paths),
        Some(Command::Edit { path }) => cmd_edit(&paths, &path),
        Some(Command::Copy { path, to }) => cmd_copy(&paths, &path, to),
        Some(Command::Rm { paths: rels }) => cmd_rm(&paths, &rels),
        Some(Command::Mv { from, to }) => cmd_mv(&paths, &from, &to),
        Some(Command::Tag { path, add, rm, set }) => cmd_tag(&paths, &path, &add, &rm, set),
        Some(Command::Describe { path, description }) => cmd_describe(&paths, &path, &description),
        Some(Command::Init { force }) => cmd_init(&paths, force),
        None => tui::run(paths),
    }
}

fn cmd_add(paths: &Paths, inputs: &[PathBuf], opts: AddOptions) -> Result<()> {
    let conn = index::db::open(paths)?;
    let mut added = 0usize;
    let mut deduped = 0usize;
    for input in inputs {
        for item in store::add(paths, input, &opts)? {
            if item.deduped {
                deduped += 1;
                println!("= {} (already present)", item.rel_path);
            } else {
                index::reindex::index_one(&conn, paths, &item.abs_path)?;
                added += 1;
                println!("+ {}", item.rel_path);
            }
        }
    }
    tracing::info!(added, deduped, "add");
    println!("done: {added} added, {deduped} duplicate(s) skipped");
    Ok(())
}

fn cmd_search(paths: &Paths, query: &str, limit: usize) -> Result<()> {
    let conn = index::db::open(paths)?;
    let results = index::search::search(&conn, query, limit)?;
    tracing::info!(
        query,
        names = results.names.len(),
        content = results.content.len(),
        "search"
    );
    if results.names.is_empty() && results.content.is_empty() {
        println!("no matches for {query:?}");
        return Ok(());
    }
    print_section("names", &results.names);
    print_section("content", &results.content);
    Ok(())
}

fn print_section(title: &str, hits: &[index::search::Hit]) {
    if hits.is_empty() {
        return;
    }
    println!("{title}:");
    for hit in hits {
        let tags = if hit.record.tags.is_empty() {
            String::new()
        } else {
            format!("  [{}]", hit.record.tags.join(", "))
        };
        let desc = if hit.record.description.is_empty() {
            String::new()
        } else {
            format!("  — {}", hit.record.description)
        };
        println!("  {}{tags}{desc}", hit.record.rel_path);
    }
}

fn cmd_edit(paths: &Paths, rel: &str) -> Result<()> {
    let config = config::Config::load(paths)?;
    if editor::edit(paths, &config, rel)? {
        store::rehash(paths, rel)?;
        let conn = index::db::open(paths)?;
        index::reindex::index_one(&conn, paths, &paths.library.join(rel))?;
        tracing::info!(rel, "edited");
        println!("edited {rel}");
    } else {
        println!("editor exited without success; nothing reindexed");
    }
    Ok(())
}

fn cmd_copy(paths: &Paths, rel: &str, to: Option<PathBuf>) -> Result<()> {
    let dest_dir = match to {
        Some(dir) => dir,
        None => std::env::current_dir()?,
    };
    let dest = reuse::copy_into_dir(paths, rel, &dest_dir)?;
    tracing::info!(rel, dest = %dest.display(), "copied");
    println!("copied to {}", dest.display());
    Ok(())
}

fn cmd_rm(paths: &Paths, rels: &[String]) -> Result<()> {
    let conn = index::db::open(paths)?;
    for rel in rels {
        store::remove(paths, rel)?;
        index::db::remove(&conn, rel)?;
        tracing::info!(rel, "removed");
        println!("removed {rel}");
    }
    Ok(())
}

fn cmd_mv(paths: &Paths, from: &str, to: &str) -> Result<()> {
    let conn = index::db::open(paths)?;
    store::rename(paths, from, to)?;
    index::db::remove(&conn, from)?;
    index::reindex::index_one(&conn, paths, &paths.library.join(to))?;
    tracing::info!(from, to, "moved");
    println!("moved {from} -> {to}");
    Ok(())
}

fn cmd_tag(
    paths: &Paths,
    rel: &str,
    add: &[String],
    rm: &[String],
    set: Option<Vec<String>>,
) -> Result<()> {
    let tags = store::edit_tags(paths, rel, add, rm, set)?;
    let conn = index::db::open(paths)?;
    index::reindex::index_one(&conn, paths, &paths.library.join(rel))?;
    tracing::info!(rel, "tagged");
    println!("tags for {rel}: [{}]", tags.join(", "));
    Ok(())
}

fn cmd_describe(paths: &Paths, rel: &str, description: &str) -> Result<()> {
    store::set_description(paths, rel, description)?;
    let conn = index::db::open(paths)?;
    index::reindex::index_one(&conn, paths, &paths.library.join(rel))?;
    tracing::info!(rel, "described");
    println!("updated description for {rel}");
    Ok(())
}

fn cmd_init(paths: &Paths, force: bool) -> Result<()> {
    if config::Config::write_default(paths, force)? {
        tracing::info!(config = %paths.config.display(), "config initialized");
        println!("wrote default config to {}", paths.config.display());
    } else {
        println!(
            "config already exists at {} (use --force to overwrite)",
            paths.config.display()
        );
    }
    Ok(())
}

fn cmd_reindex(paths: &Paths) -> Result<()> {
    let conn = index::db::open(paths)?;
    let count = index::reindex::rebuild(&conn, paths)?;
    println!("reindexed {count} entries");
    Ok(())
}

fn cmd_list(paths: &Paths) -> Result<()> {
    let conn = index::db::open(paths)?;
    let mut records = index::db::all(&conn)?;
    records.sort_by(|a, b| a.rel_path.cmp(&b.rel_path));
    if records.is_empty() {
        println!("library is empty — add something with `chimera add <path>`");
        return Ok(());
    }
    for record in &records {
        println!("{}", record.rel_path);
    }
    Ok(())
}

/// initializes stderr logging, controlled by the `CHIMERA_LOG` env var (default: warn).
fn init_tracing() {
    // controlled by CHIMERA_LOG (e.g. `info`, `debug`, `chimera=debug`); default: warn.
    let filter = std::env::var("CHIMERA_LOG")
        .ok()
        .and_then(|directives| EnvFilter::try_new(directives).ok())
        .unwrap_or_else(|| EnvFilter::new("warn"));
    let _ = tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(std::io::stderr)
        .without_time()
        .with_target(false)
        .try_init();
}
