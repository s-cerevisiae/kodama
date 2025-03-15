pub mod callback;
pub mod counter;
pub mod html_parser;
pub mod parser;
pub mod section;
pub mod state;
pub mod taxon;
pub mod typst;
pub mod writer;

use std::{collections::HashMap, path::Path};

use parser::parse_markdown;
use section::{HTMLContent, ShallowSection};
use snafu::ResultExt;
use state::CompileState;
use typst::parse_typst;
use walkdir::WalkDir;
use writer::Writer;

use crate::{
    config::{self, verify_and_file_hash},
    error::{CompileError, FileCollisonSnafu, IOSnafu},
    slug::{self, Ext},
};

pub fn compile_all(workspace_dir: &str) -> Result<(), CompileError> {
    let mut state = CompileState::new();
    let workspace = all_source_files(Path::new(workspace_dir))?;

    for (slug, ext) in &workspace.slug_exts {
        let relative_path = format!("{}.{}", slug, ext);

        let is_modified = verify_and_file_hash(&relative_path).context(IOSnafu {
            path: &relative_path,
        })?;

        let entry_path_str = format!("{}.entry", relative_path);
        let entry_path_buf = config::entry_path(&entry_path_str);

        let shallow = if !is_modified && entry_path_buf.exists() {
            let serialized = std::fs::read_to_string(entry_path_buf).context(IOSnafu {
                path: &entry_path_str,
            })?;

            let shallow: ShallowSection = serde_json::from_str(&serialized).unwrap();
            shallow
        } else {
            let shallow = match ext {
                Ext::Markdown => parse_markdown(slug)?,
                Ext::Typst => parse_typst(slug, workspace_dir)?,
            };
            let serialized = serde_json::to_string(&shallow).unwrap();
            std::fs::write(entry_path_buf, serialized).context(IOSnafu {
                path: entry_path_str,
            })?;

            shallow
        };

        state.residued.insert(slug.to_string(), shallow);
    }

    state.compile_all();

    Writer::write_needed_slugs(
        &workspace.slug_exts.into_iter().map(|x| x.0).collect(),
        &state,
    );

    Ok(())
}

pub fn should_ignored_file(path: &Path) -> bool {
    let name = path.file_name().unwrap();
    name == "README.md"
}

pub fn should_ignored_dir(path: &Path) -> bool {
    let name = path.file_name().unwrap();
    name.to_str()
        .map_or(false, |s| s.starts_with('.') || s.starts_with('_'))
}

/**
 * collect all source file paths in workspace dir
 */
pub fn all_source_files(root_dir: &Path) -> Result<Workspace, CompileError> {
    let mut slug_exts = HashMap::new();
    let to_slug_ext = |p: &Path| {
        let p = p.strip_prefix(root_dir).unwrap_or(p);
        let (slug, ext) = slug::path_to_slug(p);
        Some((slug, ext?))
    };

    for entry in std::fs::read_dir(root_dir).context(IOSnafu { path: root_dir })? {
        let path = entry.context(IOSnafu { path: root_dir })?.path();
        if path.is_file() && !should_ignored_file(&path) {
            let Some((slug, ext)) = to_slug_ext(&path) else {
                continue;
            };
            if let Some(ext) = slug_exts.insert(slug, ext) {
                return FileCollisonSnafu { path: &path, ext }.fail();
            };
        } else if path.is_dir() && !should_ignored_dir(&path) {
            for entry in WalkDir::new(&path)
                .follow_links(true)
                .into_iter()
                .filter_entry(|e| {
                    let path = e.path();
                    path.is_file() || !should_ignored_dir(path)
                })
            {
                let path = entry
                    .map_err(|e| e.into())
                    .context(IOSnafu { path: &path })?
                    .into_path();
                if path.is_file() {
                    let Some((slug, ext)) = to_slug_ext(&path) else {
                        continue;
                    };
                    if let Some(ext) = slug_exts.insert(slug, ext) {
                        return FileCollisonSnafu { path, ext }.fail();
                    }
                }
            }
        }
    }

    Ok(Workspace { slug_exts })
}

#[derive(Debug)]
pub struct Workspace {
    pub slug_exts: HashMap<String, Ext>,
}
