//! Deterministic identifier-to-object evaluation environment.

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
}

impl Environment {
    /// Creates an empty environment.
    #[must_use]
    pub fn new() -> Self {
        Self {
            objects: IndexMap::new(),
            indices: BTreeMap::new(),
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
