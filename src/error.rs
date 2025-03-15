use std::backtrace::Backtrace;

use snafu::Snafu;

#[derive(Debug, Snafu)]
#[snafu(visibility(pub(crate)))]
pub enum CompileError {
    #[snafu(display("failed to operate on file `{file}`"))]
    IO {
        file: String,
        source: std::io::Error,
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
