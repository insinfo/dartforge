//! Scalar RTI caches and metadata-only runtime effects for allocation witnesses.
//!
//! `runtime/types.rs` stores only Header.metadata in rti_set. rti_recipe interns
//! an RTI integer in the thread-local type universe; neither writes a physical
//! object CID or invokes Dart. These facts permit provenance propagation, never
//! removal or reordering of RTI operations, stores, or exception checks.
use super::*;

pub(super) struct Inventory {
    globals: HashMap<String, Type>,
    helpers: HashSet<String>,
    enabled: bool,
}
impl Inventory {
    pub(super) fn new(modules: &[&Module]) -> Self {
        let mut result = Self {
            globals: HashMap::new(),
            helpers: HashSet::new(),
            enabled: true,
        };
        let mut duplicates = HashSet::new();
        let mut declarations = HashMap::new();
        for (_, ty, symbol) in modules.iter().flat_map(|module| &module.globals) {
            declarations
                .entry(symbol.clone())
                .or_insert_with(Vec::new)
                .push(*ty);
        }
        for module in modules {
            for (_, ty, symbol) in &module.globals {
                if symbol.starts_with("dfr.") && *ty == Type::I64 {
                    if result.globals.insert(symbol.clone(), Type::I64).is_some() {
                        duplicates.insert(symbol.clone());
                    }
                    let flag = format!("{symbol}$ok");
                    if result.globals.insert(flag.clone(), Type::I8).is_some() {
                        duplicates.insert(flag);
                    }
                }
            }
            for function in &module.functions {
                if matches!(
                    function.symbol.as_str(),
                    "dartforge_rti_set"
                        | "dartforge_rti_recipe"
                        | "dartforge_globals_area"
                        | "dartforge_globals_area_id"
                ) {
                    result.enabled = false;
                }
                for (_, instruction, _) in function.blocks.iter().flat_map(|b| &b.instructions) {
                    if matches!(instruction, I::CallRuntime { name, .. } | I::CallStatic { symbol: name, .. }
                        if matches!(name.as_str(), "dartforge_globals_area" | "dartforge_globals_area_id"))
                    {
                        result.enabled = false;
                    }
                }
            }
        }
        for name in duplicates {
            result.globals.remove(&name);
        }
        result
            .globals
            .retain(|symbol, ty| match declarations.get(symbol) {
                Some(types) => types.as_slice() == [*ty],
                None => symbol.ends_with("$ok") && *ty == Type::I8,
            });
        if !result.enabled {
            return result;
        }
        for function in modules.iter().flat_map(|m| &m.functions) {
            if !function.symbol.starts_with("df.rti.")
                || function.return_ty != Type::I64
                || !function.params.is_empty()
            {
                continue;
            }
            let Ok(shape) = Shape::new(function) else {
                continue;
            };
            let mut recipes = 0;
            let allowed = function.blocks.iter().all(|block| {
                if let Terminator::Return(value) = &block.terminator {
                    if value.as_ref().and_then(|value| shape.operand_type(value)) != Some(Type::I64) { return false; }
                }
                block.instructions.iter().all(|(_, instruction, _)| match instruction {
                    I::Const(Constant::String(_) | Constant::StringWtf8(_)) | I::ICmp(..) | I::Phi { .. } => true,
                    I::CallRuntime { name, args, ret_ty: Type::I64 } if name == "dartforge_rti_recipe" => {
                        recipes += 1;
                        matches!(args.as_slice(), [(arg, Type::Ref)] if shape.operand_type(arg) == Some(Type::Ref))
                    },
                    instruction => result.scalar_access(instruction, &shape),
                })
            });
            if allowed && recipes == 1 {
                result.helpers.insert(function.symbol.clone());
            }
        }
        result
    }
    pub(super) fn helper(&self, symbol: &str) -> bool {
        self.enabled && self.helpers.contains(symbol)
    }
    pub(super) fn scalar_access(&self, instruction: &I, shape: &Shape) -> bool {
        if !self.enabled {
            return false;
        }
        match instruction {
            I::LoadGlobal { symbol, ty } => self.globals.get(symbol) == Some(ty),
            I::StoreGlobal {
                symbol,
                val,
                ty,
                root: None,
            } => {
                self.globals.get(symbol) == Some(ty)
                    && shape.operand_type(val).is_some_and(|actual| {
                        actual == *ty || (*ty == Type::I8 && actual == Type::I64)
                    })
            }
            _ => false,
        }
    }
    pub(super) fn runtime_effect(&self, instruction: &I, shape: &Shape, helper: bool) -> bool {
        if !self.enabled {
            return false;
        }
        match instruction {
            I::CallRuntime {
                name,
                args,
                ret_ty: Type::Void,
            } if name == "dartforge_rti_set" => {
                matches!(args.as_slice(), [(object, Type::Ref), (rti, Type::I64)] if shape.operand_type(object) == Some(Type::Ref) && shape.operand_type(rti) == Some(Type::I64))
            }
            I::CallRuntime {
                name,
                args,
                ret_ty: Type::I64,
            } if helper && name == "dartforge_rti_recipe" => {
                matches!(args.as_slice(), [(text, Type::Ref)] if shape.operand_type(text) == Some(Type::Ref))
            }
            _ => false,
        }
    }
}
