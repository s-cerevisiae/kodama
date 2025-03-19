use std::{fmt::Display, path::Path, str::FromStr};

use internment::Intern;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, PartialOrd, Ord, Deserialize)]
#[serde(from = "String")]
pub struct Slug(Intern<str>);

impl Slug {
    pub fn new<S: AsRef<str>>(s: S) -> Self {
        Self(s.as_ref().into())
    }

    pub fn as_str(&self) -> &'static str {
        self.0.as_ref()
    }
}

impl From<String> for Slug {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}

impl PartialEq<&str> for Slug {
    fn eq(&self, other: &&str) -> bool {
        self.as_str() == *other
    }
}

impl PartialEq<Slug> for &str {
    fn eq(&self, other: &Slug) -> bool {
        *self == other.as_str()
    }
}

impl PartialEq<Slug> for String {
    fn eq(&self, other: &Slug) -> bool {
        self == other.as_str()
    }
}

impl Serialize for Slug {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(self.0.as_ref())
    }
}

impl Display for Slug {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

#[derive(Clone, Copy, Debug)]
pub enum Ext {
    Markdown,
    Typst,
}

pub struct ParseExtensionError;

impl FromStr for Ext {
    type Err = ParseExtensionError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "md" => Ok(Self::Markdown),
            "typst" => Ok(Self::Typst),
            _ => Err(ParseExtensionError),
        }
    }
}

impl Display for Ext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Ext::Markdown => "md",
            Ext::Typst => "typst",
        };
        write!(f, "{s}")
    }
}

pub fn to_hash_id(slug_str: &str) -> String {
    slug_str.replace("/", "-")
}

/// path to slug
pub fn to_slug(fullname: &str) -> Slug {
    let path = Path::new(fullname);
    let slug = strip_base(path).with_extension("");
    Slug::new(pretty_path(&slug))
}

/// Strip `./` or `/` from a `Path` if exists.
/// Works on both Windows and \*nix.
fn strip_base(path: &Path) -> &Path {
    path
        .strip_prefix("./")
        .or_else(|_| path.strip_prefix("/"))
        .unwrap_or(path)
}

pub fn pretty_path(path: &std::path::Path) -> String {
    posix_style(clean_path(path).to_str().unwrap())
}

pub fn posix_style(s: &str) -> String {
    s.replace("\\", "/")
}

fn clean_path(path: &std::path::Path) -> std::path::PathBuf {
    let mut cleaned_path = std::path::PathBuf::new();
    for component in path.components() {
        match component {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                cleaned_path.pop();
            }
            _ => {
                cleaned_path.push(component.as_os_str());
            }
        }
    }
    cleaned_path
}

pub fn adjust_name(path: &str, expect: &str, target: &str) -> String {
    let prefix = if path.ends_with(expect) {
        &path[0..path.len() - expect.len()]
    } else {
        path
    };
    format!("{}{}", prefix, target)
}
