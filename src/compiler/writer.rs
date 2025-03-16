use std::{collections::HashSet, ops::Not, path::Path};

use crate::{
    compiler::counter::Counter,
    config::{self, verify_update_hash},
    entry::MetaData,
    html,
    html_flake::{self, html_article_inner},
};

use super::{
    callback::CallbackValue,
    section::{Section, SectionContent},
    state::CompileState,
    taxon::Taxon,
};

pub struct Writer {}

impl Writer {
    pub fn write(section: &Section, state: &CompileState) {
        let (html, page_title) = Writer::html_doc(section, state);
        let html_url = format!("{}.html", section.slug());
        let filepath = crate::config::output_path(&html_url);

        let relative_path = config::join_path(&config::output_dir(), &html_url);
        if verify_update_hash(&relative_path, &html).expect("Writer::write@hash") {
            match std::fs::write(&filepath, html) {
                Ok(()) => {
                    let output_path = crate::slug::pretty_path(Path::new(&html_url));
                    println!("Output: {:?} {}", page_title, output_path);
                }
                Err(err) => eprintln!("{:?}", err),
            }
        }
    }

    pub fn write_needed_slugs(all_slugs: &Vec<String>, state: &CompileState) {
        all_slugs
            .iter()
            .for_each(|slug| match state.compiled().get(slug) {
                /*
                 * No need for `state.compiled.remove(slug)` here,
                 * because writing to a file does not require a mutable reference
                 * of the [`Section`].
                 */
                None => eprintln!("Slug `{}` not in compiled entries.", slug),
                Some(section) => Writer::write(section, &state),
            });
    }

    pub fn html_doc(section: &Section, state: &CompileState) -> (String, String) {
        let mut counter = Counter::init();

        let (article_inner, items) = Writer::section_to_html(section, &mut counter, true, false);
        let catalog_html = items
            .is_empty()
            .not()
            .then(|| Writer::catalog_block(&items))
            .unwrap_or_default();

        let slug = section.slug();
        let html_header = Writer::header(state, &slug);

        let callback = state.callback().0.get(&slug);
        let footer_html = Writer::footer(state, &section.references, callback);
        let page_title = section.metadata.page_title().map_or("", |s| s.as_str());

        let html = crate::html_flake::html_doc(
            &page_title,
            &html_header,
            &article_inner,
            &footer_html,
            &catalog_html,
        );

        (html, page_title.to_string())
    }

    fn header(state: &CompileState, slug: &str) -> String {
        state
            .callback()
            .0
            .get(slug)
            .and_then(|callback| {
                let parent = &callback.parent;
                state.compiled().get(parent).map(|section| {
                    let href = config::full_html_url(parent);
                    let title = section.metadata.title().map_or("", |s| s);
                    let page_title = section.metadata.page_title().map_or("", |s| s);
                    html_flake::html_header_nav(title, page_title, &href)
                })
            })
            .unwrap_or_default()
    }

    fn footer(
        state: &CompileState,
        references: &HashSet<String>,
        callback: Option<&CallbackValue>,
    ) -> String {
        let mut references: Vec<&String> = references.iter().collect();
        references.sort();

        let references_html = references
            .iter()
            .map(|slug| {
                let slug = slug.to_string();
                let section = state.compiled().get(&slug).unwrap();
                Writer::footer_section_to_html(section)
            })
            .reduce(|s, t| s + &t)
            .map(|s| html_flake::html_footer_section("References", &s))
            .unwrap_or_default();

        let backlinks_html = callback
            .map(|s| {
                let mut backlinks: Vec<&String> = s.backlinks.iter().collect();
                backlinks.sort();
                backlinks
                    .iter()
                    .map(|slug| {
                        let slug = Writer::clip_metadata_badge(slug);
                        let section = state.compiled().get(&slug).unwrap();
                        Writer::footer_section_to_html(section)
                    })
                    .reduce(|s, t| s + &t)
                    .map(|s| html_flake::html_footer_section("Backlinks", &s))
                    .unwrap_or_default()
            })
            .unwrap_or_default();

        html!(footer => (references_html) (backlinks_html))
    }

    fn clip_metadata_badge(slug: &str) -> String {
        match slug.ends_with(":metadata") {
            true => slug[0..slug.len() - ":metadata".len()].to_string(),
            false => slug.to_string(),
        }
    }

    fn catalog_block(items: &str) -> String {
        html!(div class = "block" =>
          (html!(h1 => "Table of Contents")) (items))
    }

    fn catalog_item(section: &Section, taxon: &str, child_html: &str) -> String {
        let slug = &section.slug();
        let title = section.metadata.title().map_or("", |s| s);
        let page_title = section.metadata.page_title().map_or("", |s| s);
        html_flake::catalog_item(
            slug,
            title,
            page_title,
            section.option.details_open,
            taxon,
            child_html,
        )
    }

    fn footer_content_to_html(content: &SectionContent) -> String {
        match content {
            SectionContent::Plain(s) => s.to_string(),
            SectionContent::Embed(section) => Writer::footer_section_to_html(section),
        }
    }

    fn footer_section_to_html(section: &Section) -> String {
        match config::footer_mode() {
            config::FooterMode::Link => {
                let summary = section.metadata.to_header(None, None);
                format!(r#"<section class="block">{summary}</section>"#)
            }
            config::FooterMode::Embed => {
                let contents = match section.children.len() > 0 {
                    false => String::new(),
                    true => section
                        .children
                        .iter()
                        .map(Writer::footer_content_to_html)
                        .reduce(|s, t| s + &t)
                        .unwrap(),
                };
                html_article_inner(&section.metadata, &contents, false, false, None, None)
            }
        }
    }

    pub fn section_to_html(
        section: &Section,
        counter: &mut Counter,
        toplevel: bool,
        hide_metadata: bool,
    ) -> (String, String) {
        let adhoc_taxon = Writer::taxon(section, counter);
        let (contents, items) = match section.children.len() > 0 {
            false => (String::new(), String::new()),
            true => {
                let mut subcounter = match section.option.numbering {
                    true => counter.left_shift(),
                    false => counter.clone(),
                };
                let content_to_html = |c: &SectionContent| {
                    let is_collection = section.metadata.is_collect();
                    Writer::content_to_html(c, &mut subcounter, !is_collection)
                };
                section
                    .children
                    .iter()
                    .map(content_to_html)
                    .reduce(|s, t| (s.0 + &t.0, s.1 + &t.1))
                    .unwrap()
            }
        };

        let child_html = items
            .is_empty()
            .not()
            .then(|| format!(r#"<ul class="block">{}</ul>"#, &items))
            .unwrap_or_default();

        let catalog_item = match toplevel {
            true => child_html,
            false => section
                .option
                .catalog
                .then(|| Writer::catalog_item(section, &adhoc_taxon, &child_html))
                .unwrap_or(String::new()),
        };

        let article_inner = html_article_inner(
            &section.metadata,
            &contents,
            hide_metadata,
            section.option.details_open,
            None,
            Some(adhoc_taxon.as_str()),
        );

        (article_inner, catalog_item)
    }

    fn content_to_html(
        content: &SectionContent,
        counter: &mut Counter,
        hide_metadata: bool,
    ) -> (String, String) {
        match content {
            SectionContent::Plain(s) => (s.to_string(), String::new()),
            SectionContent::Embed(section) => {
                Writer::section_to_html(section, counter, false, hide_metadata)
            }
        }
    }

    fn taxon(section: &Section, counter: &mut Counter) -> String {
        if section.option.numbering {
            counter.step_mut();
            let numbering = Some(counter.display());
            let text = section.metadata.taxon().map_or("", |s| s);
            let taxon = Taxon::new(numbering, text.to_string());
            return taxon.display();
        }
        section.metadata.taxon().map_or("", |s| s).to_string()
    }
}
