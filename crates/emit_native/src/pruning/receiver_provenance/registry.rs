//! Detect explicit runtime registry mutation through lowered native aliases.
use super::*;

fn mutates_registry(name: &str) -> bool {
    matches!(
        name,
        "dartforge_register_methods"
            | "dartforge_register_table"
            | "dartforge_register_cids"
            | "dartforge_register_subclass"
            | "dartforge_set_migration"
    )
}

pub(crate) fn closed(modules: &[&Module]) -> bool {
    for function in modules.iter().flat_map(|m| &m.functions) {
        if mutates_registry(&function.symbol) {
            return false;
        }
        let arrays: HashMap<_, _> = function
            .blocks
            .iter()
            .flat_map(|b| &b.instructions)
            .filter_map(|(value, instruction, _)| match instruction {
                I::ConstArray(words) => Some((
                    *value,
                    words
                        .iter()
                        .flat_map(|word| word.to_le_bytes())
                        .collect::<Vec<_>>(),
                )),
                _ => None,
            })
            .collect();
        for (_, instruction, _) in function.blocks.iter().flat_map(|b| &b.instructions) {
            match instruction {
                I::CallRuntime { name, .. } | I::CallStatic { symbol: name, .. }
                    if mutates_registry(name) =>
                {
                    return false;
                }
                I::Const(Constant::String(name)) if mutates_registry(name) => return false,
                I::CallRuntime { name, args, .. } if name == "dartforge_ffi_native_symbol" => {
                    let [
                        (Operand::Val(pointer), Type::Ptr),
                        (Operand::Constant(Constant::Int(length)), Type::I64),
                    ] = args.as_slice()
                    else {
                        return false;
                    };
                    let Some(bytes) = arrays.get(pointer) else {
                        return false;
                    };
                    let Some(length) = usize::try_from(*length).ok().filter(|n| *n <= bytes.len())
                    else {
                        return false;
                    };
                    let Ok(symbol) = std::str::from_utf8(&bytes[..length]) else {
                        return false;
                    };
                    if mutates_registry(symbol) {
                        return false;
                    }
                }
                _ => {}
            }
        }
    }
    true
}
