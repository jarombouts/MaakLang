//! The binding environment: named, typed, mutable values. Turtles are just bindings whose
//! value is a `Schildpad` handle — "variables in a turtle costume" (LANGUAGE.md §1). The
//! turtle *state* lives in the engine's turtle store; the binding holds the handle.

use alloc::collections::BTreeMap;
use alloc::string::String;

use crate::value::Value;
use crate::vocab::Type;

#[derive(Debug, Clone)]
pub struct Binding {
    pub ty: Type,
    pub value: Value,
    pub name: String, // the display name as the child wrote it
}

#[derive(Debug, Clone, Default)]
pub struct Env {
    map: BTreeMap<String, Binding>,
}

impl Env {
    pub fn new() -> Self {
        Env { map: BTreeMap::new() }
    }

    fn key(name: &str) -> String {
        name.to_lowercase()
    }

    pub fn get(&self, name: &str) -> Option<&Binding> {
        self.map.get(&Self::key(name))
    }

    pub fn get_mut(&mut self, name: &str) -> Option<&mut Binding> {
        self.map.get_mut(&Self::key(name))
    }

    pub fn has(&self, name: &str) -> bool {
        self.map.contains_key(&Self::key(name))
    }

    pub fn set(&mut self, binding: Binding) {
        self.map.insert(Self::key(&binding.name), binding);
    }

    pub fn clear(&mut self) {
        self.map.clear();
    }
}
