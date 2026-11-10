//! Variable, procedure, and function tables.
//!
//! `.fox` is COSY, so `Foo` and `FOO` are one name. The key stays the spelling
//! of the first declaration, which is also the Rust binding. `.rosy` stays
//! case-sensitive.

use std::collections::HashMap;

use crate::syntax_config;

#[derive(Debug, Clone)]
pub struct NameMap<V> {
    map: HashMap<String, V>,
}

impl<V> Default for NameMap<V> {
    fn default() -> Self {
        Self {
            map: HashMap::new(),
        }
    }
}

impl<V> NameMap<V> {
    fn key_ref<'a>(&'a self, name: &str) -> Option<&'a str> {
        if let Some((key, _)) = self.map.get_key_value(name) {
            return Some(key.as_str());
        }
        if !syntax_config::is_cosy_syntax() {
            return None;
        }
        self.map
            .keys()
            .find(|k| k.eq_ignore_ascii_case(name))
            .map(String::as_str)
    }

    /// Spelling of the declaration that `name` refers to.
    pub fn declared_key<'a>(&'a self, name: &str) -> Option<&'a str> {
        self.key_ref(name)
    }

    pub fn get(&self, name: &str) -> Option<&V> {
        self.key_ref(name).and_then(|k| self.map.get(k))
    }

    pub fn get_mut(&mut self, name: &str) -> Option<&mut V> {
        let key = self.key_ref(name)?.to_string();
        self.map.get_mut(&key)
    }

    pub fn contains_key(&self, name: &str) -> bool {
        self.key_ref(name).is_some()
    }

    pub fn insert(&mut self, name: String, value: V) -> Option<V> {
        if let Some(existing) = self.key_ref(&name) {
            let existing = existing.to_string();
            self.map.insert(existing, value)
        } else {
            self.map.insert(name, value)
        }
    }

    pub fn keys(&self) -> impl Iterator<Item = &String> {
        self.map.keys()
    }

    pub fn values_mut(&mut self) -> impl Iterator<Item = &mut V> {
        self.map.values_mut()
    }
}
