use std::collections::{BTreeSet, HashMap, HashSet};

use crate::{
    config,
    entry::{EntryMetaData, HTMLMetaData, MetaData, KEY_SLUG},
    slug,
};

use super::{
    callback::Callback,
    section::{HTMLContent, LazyContent, Section, SectionContent, SectionContents, ShallowSection},
    taxon::Taxon,
};

#[derive(Debug)]
pub struct CompileState {
    residued: BTreeSet<String>,
    compiled: HashMap<String, Section>,
    callback: Callback,
}

type Shallows = HashMap<String, ShallowSection>;

pub fn compile_all(mut shallows: Shallows) -> CompileState {
    for shallow in shallows.values_mut() {
        shallow.metadata.compute_textual_attrs();
    }

    let residued: BTreeSet<String> = shallows.keys().cloned().collect();

    let mut state = CompileState::new(residued);
    state.compile(&shallows, "index");

    /*
     * Unlinked or unembedded pages.
     */
    while let Some(slug) = state.residued.pop_first() {
        state.compile(&shallows, &slug);
    }

    state
}

impl CompileState {
    fn new(residued: BTreeSet<String>) -> CompileState {
        CompileState {
            residued,
            compiled: HashMap::new(),
            callback: Callback::new(),
        }
    }

    fn compile(&mut self, shallows: &Shallows, slug: &str) -> &Section {
        self.fetch_section(shallows, slug).unwrap()
    }

    fn fetch_section(&mut self, shallows: &Shallows, slug: &str) -> Option<&Section> {
        if self.compiled.contains_key(slug) {
            Some(self.compiled.get(slug).unwrap())
        } else {
            shallows
                .get(slug)
                .map(|shallow| self.compile_shallow(shallows, shallow))
        }
    }

    fn compile_shallow(&mut self, shallows: &Shallows, shallow: &ShallowSection) -> &Section {
        let slug = shallow.slug();
        let mut children: SectionContents = vec![];
        let mut references: HashSet<String> = HashSet::new();

        match &shallow.content {
            HTMLContent::Plain(html) => {
                children.push(SectionContent::Plain(html.to_string()));
            }
            HTMLContent::Lazy(lazy_contents) => {
                let mut callback: Callback = Callback::new();

                for lazy_content in lazy_contents {
                    match lazy_content {
                        LazyContent::Plain(html) => {
                            children.push(SectionContent::Plain(html.to_string()));
                        }
                        LazyContent::Embed(embed_content) => {
                            let child_slug = slug::to_slug(&embed_content.url);
                            let refered = match self.fetch_section(shallows, &child_slug) {
                                Some(refered_section) => refered_section,
                                None => {
                                    eprintln!(
                                        "Error: [{}] attempting to fetch a non-existent [{}].",
                                        slug, child_slug,
                                    );
                                    continue;
                                }
                            };

                            if embed_content.option.details_open {
                                references.extend(refered.references.clone());
                            }
                            callback.insert_parent(child_slug, slug.to_string());

                            let mut child_section = refered.clone();
                            child_section.option = embed_content.option.clone();
                            if let Some(title) = &embed_content.title {
                                child_section
                                    .metadata
                                    .update("title".to_string(), title.to_string())
                            };
                            children.push(SectionContent::Embed(child_section));
                        }
                        LazyContent::Local(local_link) => {
                            let link_slug = &local_link.slug;
                            let article_title = get_metadata(shallows, link_slug)
                                .map_or("", |s| s.page_title().map_or("", |s| s));

                            if is_reference(shallows, link_slug) {
                                references.insert(link_slug.to_string());
                            }

                            /*
                             * Making oneself the content of a backlink should not be expected behavior.
                             */
                            if *link_slug != slug
                                && format!("{}:metadata", link_slug) != slug
                                && is_enable_backlinks(shallows, link_slug)
                            {
                                callback.insert_backlinks(
                                    link_slug.to_string(),
                                    vec![slug.to_string()],
                                );
                            }

                            let local_link = local_link.text.clone();
                            let text = local_link.unwrap_or(article_title.to_string());

                            let html = crate::html_flake::html_link(
                                &config::full_html_url(link_slug),
                                &format!("{} [{}]", article_title, link_slug),
                                &text,
                                crate::recorder::State::LocalLink.strify(),
                            );
                            children.push(SectionContent::Plain(html.to_string()));
                        }
                    }
                }

                self.callback.merge(callback);
            }
        };

        // compile metadata
        let mut metadata = EntryMetaData(HashMap::new());
        metadata.update(KEY_SLUG.to_string(), slug.to_string());
        shallow.metadata.keys().for_each(|key| {
            if key == KEY_SLUG {
                return;
            }
            let value = shallow.metadata.get(key).unwrap();
            let spanned: ShallowSection = Self::metadata_to_section(value, &slug);
            let compiled = self.compile_shallow(shallows, &spanned);
            let html = compiled.spanned();
            metadata.update(key.to_string(), html);
        });

        // remove from `self.residued` after compiled.
        self.residued.remove(&slug);

        let section = Section::new(metadata, children, references);
        self.compiled.insert(slug.to_string(), section);
        self.compiled.get(&slug).unwrap()
    }

    fn metadata_to_section(content: &HTMLContent, current_slug: &str) -> ShallowSection {
        let mut metadata = HashMap::new();
        metadata.insert(
            KEY_SLUG.to_string(),
            HTMLContent::Plain(format!("{}:metadata", current_slug)),
        );

        ShallowSection {
            metadata: HTMLMetaData(metadata),
            content: content.clone(),
        }
    }

    pub fn compiled(&self) -> &HashMap<String, Section> {
        &self.compiled
    }

    pub fn callback(&self) -> &Callback {
        &self.callback
    }
}

fn get_metadata<'s>(shallows: &'s Shallows, slug: &str) -> Option<&'s HTMLMetaData> {
    shallows.get(slug).map(|s| &s.metadata)
}

fn is_enable_backlinks(shallows: &Shallows, slug: &str) -> bool {
    shallows
        .get(slug)
        .map(|s| s.metadata.is_enable_backlinks())
        .unwrap_or(true)
}

fn is_reference(shallows: &Shallows, slug: &str) -> bool {
    shallows
        .get(slug)
        .map(|s| {
            let metadata = &s.metadata;
            metadata.is_asref()
                || Taxon::is_reference(metadata.data_taxon().map_or("", String::as_str))
        })
        .unwrap_or(false)
}
