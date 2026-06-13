use std::path::PathBuf;

use clap::{Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(
    name = "chimera",
    version,
    about = "personal arsenal: store, search and reuse your code artifacts"
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// add a file or directory (recursively) to the library
    Add {
        /// files or directories to add
        #[arg(required = true)]
        paths: Vec<PathBuf>,
        /// destination category (sub-directory); inferred from the extension when omitted
        #[arg(short, long)]
        category: Option<String>,
        /// comma-separated tags
        #[arg(short, long, value_delimiter = ',')]
        tags: Vec<String>,
        /// short description
        #[arg(short, long)]
        description: Option<String>,
    },
    /// search the library
    Search {
        /// query: free text, a fuzzy name, or a glob like "*.sh"
        query: String,
        /// maximum number of results
        #[arg(short, long, default_value_t = 20)]
        limit: usize,
    },
    /// rebuild the search index from the library on disk
    Reindex,
    /// list every entry in the library
    List,
    /// open an entry in $EDITOR (e.g. `chimera edit bash/deploy.sh`)
    Edit {
        /// library-relative path of the entry
        path: String,
    },
    /// copy an entry into the current directory (or --to)
    Copy {
        /// library-relative path of the entry
        path: String,
        /// destination directory (defaults to the current directory)
        #[arg(short, long)]
        to: Option<PathBuf>,
    },
    /// remove one or more entries from the library
    Rm {
        /// library-relative paths to remove
        #[arg(required = true)]
        paths: Vec<String>,
    },
    /// move or rename an entry within the library
    Mv {
        /// current library-relative path
        from: String,
        /// new library-relative path
        to: String,
    },
    /// edit an entry's tags
    Tag {
        /// library-relative path
        path: String,
        /// tags to add (comma-separated)
        #[arg(short, long, value_delimiter = ',')]
        add: Vec<String>,
        /// tags to remove (comma-separated)
        #[arg(short, long, value_delimiter = ',')]
        rm: Vec<String>,
        /// replace all tags (comma-separated)
        #[arg(short, long, value_delimiter = ',')]
        set: Option<Vec<String>>,
    },
    /// set an entry's description
    Describe {
        /// library-relative path
        path: String,
        /// description text
        description: String,
    },
}
