use std::{backtrace::Backtrace, path::PathBuf};

use snafu::Snafu;

use crate::slug::Ext;

#[derive(Debug, Snafu)]
#[snafu(visibility(pub(crate)))]
pub enum CompileError {
    #[snafu(display("failed to operate on file `{}`", path.display()))]
    IO {
        path: PathBuf,
        source: std::io::Error,
        backtrace: Option<Backtrace>,
    },
    #[snafu(display("`{}` collides with `{}`", path.display(), path.with_extension(ext.to_string()).display()))]
    FileCollison {
        path: PathBuf,
        ext: Ext,
        backtrace: Option<Backtrace>,
    },
    #[snafu(display("failed to deserialize entry `{}`", path.display()))]
    DeserializeEntry {
        path: PathBuf,
        source: serde_json::Error,
        backtrace: Option<Backtrace>,
    },
    #[snafu(display("failed to parse file `{file}`"))]
    Syntax {
        file: String,
        #[snafu(backtrace)]
        source: SyntaxError,
    },
}

#[derive(Debug, Snafu)]
#[snafu(visibility(pub(crate)))]
pub enum SyntaxError {
    #[snafu(display("no attribute `{attr_name}` in a kodama tag"))]
    MissingAttr {
        attr_name: String,
        backtrace: Option<Backtrace>,
    },
}
