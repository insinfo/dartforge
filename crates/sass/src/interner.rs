//! Interning of names (variables, functions, mixins, properties).
//!
//! The original used `lasso::Rodeo` in a `thread_local` and returned
//! `&'a str` through a raw pointer (`unsafe`). Here each new string is
//! leaked once (`Box::leak`) and the index points to it: `Rodeo` also never
//! frees what it interned, so the memory cost is the same, and the
//! `&'static str` comes out without `unsafe` (the workspace denies
//! `unsafe_code`).
use std::cell::RefCell;
use std::collections::HashMap;
use std::fmt::{self, Display};

#[derive(Default)]
struct Table {
    indices: HashMap<&'static str, u32>,
    strings: Vec<&'static str>,
}

thread_local!(static STRINGS: RefCell<Table> = RefCell::new(Table::default()));

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Ord, PartialOrd)]
pub struct InternedString(u32);

impl InternedString {
    pub fn get_or_intern<T: AsRef<str>>(s: T) -> Self {
        let s = s.as_ref();
        STRINGS.with(|t| {
            let mut t = t.borrow_mut();
            if let Some(&i) = t.indices.get(s) {
                return Self(i);
            }
            let leaked: &'static str = Box::leak(s.to_owned().into_boxed_str());
            let i = t.strings.len() as u32;
            t.strings.push(leaked);
            t.indices.insert(leaked, i);
            Self(i)
        })
    }

    #[allow(dead_code)]
    pub fn resolve(self) -> String {
        self.resolve_ref().to_owned()
    }

    #[allow(dead_code)]
    pub fn is_empty(self) -> bool {
        self.resolve_ref().is_empty()
    }

    pub fn resolve_ref<'a>(self) -> &'a str {
        STRINGS.with(|t| t.borrow().strings[self.0 as usize])
    }
}

impl Display for InternedString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.resolve_ref())
    }
}
