//! Scalar memory reads do not allocate; pointer/value exposure is independent.
//! Only complete canonical instructions qualify, never an opcode prefix.

fn name_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || b"_$.-".contains(&byte)
}

fn decimal_end(bytes: &[u8], mut index: usize) -> usize {
    while index < bytes.len() && bytes[index].is_ascii_digit() {
        index += 1;
    }
    index
}

pub(super) fn tokens(text: &str) -> Option<Vec<&str>> {
    let bytes = text.as_bytes();
    let mut index = 0;
    let mut result = Vec::new();
    while index < bytes.len() {
        if bytes[index].is_ascii_whitespace() {
            index += 1;
            continue;
        }
        let start = index;
        if matches!(bytes[index], b'@' | b'%') {
            index += 1;
            if bytes.get(index) == Some(&b'"') {
                index = super::quoted_end(bytes, index)?;
            } else if bytes.get(index).is_some_and(u8::is_ascii_digit) {
                index = decimal_end(bytes, index);
            } else {
                let name = index;
                while index < bytes.len() && name_byte(bytes[index]) {
                    index += 1;
                }
                if index == name {
                    return None;
                }
            }
        } else if bytes[index] == b'"' {
            index = super::quoted_end(bytes, index)?;
        } else if bytes[index].is_ascii_digit() || matches!(bytes[index], b'-' | b'+' | b'.') {
            if matches!(bytes[index], b'-' | b'+') {
                index += 1;
            }
            if bytes.get(index..index + 2) == Some(b"0x") {
                return None;
            }
            let first_digit = index;
            index = decimal_end(bytes, index);
            let mut digits = index - first_digit;
            if bytes.get(index) == Some(&b'.') {
                let first = index + 1;
                index = decimal_end(bytes, first);
                digits += index - first;
            }
            if digits == 0 {
                return None;
            }
            if matches!(bytes.get(index), Some(b'e' | b'E')) {
                index += 1;
                if matches!(bytes.get(index), Some(b'+' | b'-')) {
                    index += 1;
                }
                let digits = index;
                index = decimal_end(bytes, index);
                if digits == index {
                    return None;
                }
            }
        } else if name_byte(bytes[index]) {
            while index < bytes.len() && name_byte(bytes[index]) {
                index += 1;
            }
            if text[start..index].starts_with("u0x") || text[start..index].starts_with("s0x") {
                return None;
            }
        } else {
            if !bytes[index].is_ascii() {
                return None;
            }
            index += 1;
        }
        result.push(&text[start..index]);
    }
    Some(result)
}

pub(super) fn assignment(left: &str) -> bool {
    tokens(left)
        .is_some_and(|tokens| matches!(tokens.as_slice(), [value] if value.starts_with('%')))
}

/// An LLVM instruction may follow another one without a newline, even directly
/// after a quoted SSA name. Do not hide a write/call behind a harmless prefix.
pub(super) fn embedded_effect(operation: &str) -> bool {
    const EFFECTS: &[&str] = &[
        "store",
        "call",
        "invoke",
        "callbr",
        "atomicrmw",
        "cmpxchg",
        "fence",
        "va_arg",
        "resume",
        "landingpad",
        "catchpad",
        "cleanuppad",
        "catchswitch",
        "catchret",
        "cleanupret",
        "load",
        "getelementptr",
        "inttoptr",
        "ptrtoint",
        "bitcast",
        "=",
    ];
    if !EFFECTS.iter().any(|word| operation.contains(word)) {
        return false;
    }
    let Some(tokens) = tokens(operation) else {
        return true;
    };
    tokens.iter().skip(1).any(|token| EFFECTS.contains(token))
}

fn width(ty: &str) -> Option<u32> {
    match ty {
        "i1" => Some(1),
        "i8" => Some(8),
        "i16" => Some(16),
        "i32" | "float" => Some(32),
        "i64" | "double" | "ptr" => Some(64),
        _ => None,
    }
}

fn integer(ty: &str) -> bool {
    matches!(ty, "i1" | "i8" | "i16" | "i32" | "i64")
}

pub(super) fn operand(value: &str, ty: &str) -> bool {
    if value.starts_with('%') || value.starts_with('@') || matches!(value, "undef" | "poison") {
        return true;
    }
    if ty == "ptr" {
        return value == "null";
    }
    if !integer(ty) {
        return false;
    }
    let Some(bits) = width(ty) else {
        return false;
    };
    value
        .parse::<i128>()
        .is_ok_and(|value| value >= -(1i128 << (bits - 1)) && value < (1i128 << bits))
}

/// Read-only address operations keep no implicit allocation obligation. Stores,
/// atomics, aggregate forms, metadata and unsupported syntax remain unproved.
pub(super) fn allocation_neutral(operation: &str) -> bool {
    let Some(tokens) = tokens(operation) else {
        return false;
    };
    match tokens.first().copied() {
        Some("load") => {
            let start = 1 + usize::from(tokens.get(1) == Some(&"volatile"));
            let Some([ty, ",", "ptr", pointer]) = tokens.get(start..start + 4) else {
                return false;
            };
            if width(ty).is_none() || !operand(pointer, "ptr") {
                return false;
            }
            match &tokens[start + 4..] {
                [] => true,
                [",", "align", alignment] => {
                    alignment.parse::<u32>().is_ok_and(u32::is_power_of_two)
                }
                _ => false,
            }
        }
        Some("getelementptr") => {
            let start = 1 + usize::from(tokens.get(1) == Some(&"inbounds"));
            let Some([ty, ",", "ptr", pointer]) = tokens.get(start..start + 4) else {
                return false;
            };
            let tail = &tokens[start + 4..];
            width(ty).is_some()
                && operand(pointer, "ptr")
                && !tail.is_empty()
                && tail.len().is_multiple_of(3)
                && tail
                    .as_chunks::<3>()
                    .0
                    .iter()
                    .all(|part| part[0] == "," && integer(part[1]) && operand(part[2], part[1]))
        }
        Some("inttoptr" | "ptrtoint" | "bitcast") => {
            let [opcode, from, value, "to", to] = tokens.as_slice() else {
                return false;
            };
            if !operand(value, from) {
                return false;
            }
            match *opcode {
                "inttoptr" => integer(from) && *to == "ptr",
                "ptrtoint" => *from == "ptr" && integer(to),
                _ => {
                    width(from).is_some()
                        && width(from) == width(to)
                        && (*from == "ptr") == (*to == "ptr")
                }
            }
        }
        _ => false,
    }
}
