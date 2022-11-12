//! This module is a small memory optimization for storing large amounts of potentially repetitive strings.
//! It assigns an index to each unique string, and then only the indexes are stored.
//!
//! It also uses NonZeroU32 instead of just u32 for extra optimizations that the compiler can make such as in Option<>.

use std::{
    collections::{BTreeMap, HashMap},
    num::NonZeroU32,
};

#[derive(Debug, Hash, Eq, PartialEq, Copy, Clone, Ord, PartialOrd)]
pub struct LookupId(NonZeroU32);

pub struct StringLookup {
    lookup: BTreeMap<LookupId, String>,
}

impl StringLookup {
    pub fn get(&self, id: LookupId) -> &str {
        &self.lookup.get(&id).unwrap()
    }
}

pub struct StringLookupBuilder<'a> {
    lookup: HashMap<LookupId, &'a str>,
    reverse_lookup: BTreeMap<&'a str, LookupId>,
    next_id: u32,
}

impl<'a> StringLookupBuilder<'a> {
    pub fn new() -> Self {
        Self {
            lookup: HashMap::new(),
            reverse_lookup: BTreeMap::new(),
            next_id: 1,
        }
    }

    fn get_next_id(&mut self) -> LookupId {
        let id = self.next_id;
        self.next_id += 1;
        LookupId(NonZeroU32::new(id).unwrap())
    }

    pub fn get_id(&mut self, string: &'a str) -> LookupId {
        if let Some(id) = self.reverse_lookup.get(string) {
            return *id;
        }

        let id = self.get_next_id();

        self.lookup.insert(id, string);
        self.reverse_lookup.insert(string, id);

        id
    }

    pub fn build(self) -> StringLookup {
        StringLookup {
            lookup: self
                .lookup
                .iter()
                .map(|(k, v)| (*k, v.to_string()))
                .collect(),
        }
    }
}
