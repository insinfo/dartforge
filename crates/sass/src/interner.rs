//! Internamento de nomes (variáveis, funções, mixins, propriedades).
//!
//! O original usava `lasso::Rodeo` num `thread_local` e devolvia `&'a str`
//! por um ponteiro cru (`unsafe`). Aqui cada texto novo é vazado uma vez
//! (`Box::leak`) e o índice aponta para ele: o `Rodeo` também nunca libera
//! o que internou, então o custo de memória é o mesmo, e o `&'static str`
//! sai sem `unsafe` (o workspace nega `unsafe_code`).
use std::cell::RefCell;
use std::collections::HashMap;
use std::fmt::{self, Display};

#[derive(Default)]
struct Tabela {
    indices: HashMap<&'static str, u32>,
    textos: Vec<&'static str>,
}

thread_local!(static STRINGS: RefCell<Tabela> = RefCell::new(Tabela::default()));

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
            let fixo: &'static str = Box::leak(s.to_owned().into_boxed_str());
            let i = t.textos.len() as u32;
            t.textos.push(fixo);
            t.indices.insert(fixo, i);
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
        STRINGS.with(|t| t.borrow().textos[self.0 as usize])
    }
}

impl Display for InternedString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.resolve_ref())
    }
}
