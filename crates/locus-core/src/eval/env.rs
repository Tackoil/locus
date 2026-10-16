//! Deterministic identifier-to-object evaluation environment.

use crate::scene::ResolvedPath;
use std::collections::BTreeMap;

use indexmap::IndexMap;
use locus_syntax::Ident;
use thiserror::Error;

use crate::scene::ResolvedObject;

/// Environment insertion failure.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum EnvError {
    /// An id was inserted more than once.
    #[error("duplicate object id `{0}`")]
    Duplicate(String),
}

/// Resolved objects in source order, with deterministic name lookup.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Environment {
    objects: IndexMap<Ident, ResolvedObject>,
    indices: BTreeMap<String, usize>,
    aliases: BTreeMap<String, String>,
}

impl Environment {
    pub(crate) fn set_aliases(&mut self, aliases: &BTreeMap<String, String>) {
        self.aliases.clone_from(aliases);
    }

    pub(crate) fn identity<'a>(&'a self, name: &'a str) -> &'a str {
        self.aliases.get(name).map_or(name, String::as_str)
    }
    pub(crate) fn include_path_label_bounds(&mut self, aliases: &BTreeMap<String, String>) {
        let labels = self
            .objects
            .values()
            .filter_map(|object| match object {
                ResolvedObject::Annotation(crate::scene::AnnotationSpec::Label {
                    path: Some(path),
                    bounds: Some(bounds),
                    ..
                }) => Some((path.path.clone(), *bounds)),
                _ => None,
            })
            .collect::<Vec<_>>();
        let mut combined = BTreeMap::<String, crate::BBox>::new();
        for (path, bounds) in labels {
            let identity = aliases.get(&path).unwrap_or(&path);
            combined
                .entry(identity.clone())
                .and_modify(|old| *old = old.union(bounds))
                .or_insert(bounds);
        }
        for (id, object) in &mut self.objects {
            let identity = aliases.get(&id.name).unwrap_or(&id.name);
            if let Some(bounds) = combined.get(identity)
                && let ResolvedObject::Path(ResolvedPath { label_bounds, .. }) = object
            {
                *label_bounds = Some(*bounds);
            }
        }
    }
    /// Creates an empty environment.
    #[must_use]
    pub fn new() -> Self {
        Self {
            objects: IndexMap::new(),
            indices: BTreeMap::new(),
            aliases: BTreeMap::new(),
        }
    }

    /// Inserts one source declaration at the end of the environment.
    pub fn insert(&mut self, id: Ident, object: ResolvedObject) -> Result<(), EnvError> {
        if self.indices.contains_key(&id.name) {
            return Err(EnvError::Duplicate(id.name));
        }
        let index = self.objects.len();
        self.indices.insert(id.name.clone(), index);
        self.objects.insert(id, object);
        Ok(())
    }

    /// Looks up a previously inserted object by source spelling.
    #[must_use]
    pub fn get(&self, name: &str) -> Option<&ResolvedObject> {
        self.indices
            .get(name)
            .and_then(|index| self.objects.get_index(*index))
            .map(|(_, object)| object)
    }

    /// Returns the original identifier carrying its declaration span.
    #[must_use]
    pub fn ident(&self, name: &str) -> Option<&Ident> {
        self.indices
            .get(name)
            .and_then(|index| self.objects.get_index(*index))
            .map(|(id, _)| id)
    }

    /// Borrows all objects in file order.
    #[must_use]
    pub const fn objects(&self) -> &IndexMap<Ident, ResolvedObject> {
        &self.objects
    }

    /// Consumes the environment and returns its ordered objects.
    #[must_use]
    pub fn into_objects(self) -> IndexMap<Ident, ResolvedObject> {
        self.objects
    }
}
