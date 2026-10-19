//! Owned observations from parsed LLVM modules, before code generation/LTO.
//! These records do not certify native sections or authorize managed images.

/// Array storage without expanding a potentially large zero initializer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Bytes {
    /// Explicit bytes copied out of LLVM-owned constant storage.
    Data(Vec<u8>),
    /// Logical array length; no buffer of this size was allocated.
    Zeroes(u64),
}

/// A structurally decoded initializer, without assigning a managed class.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Initializer {
    /// The exact two-word zero anchor shape.
    ZeroAnchor,
    /// Three i64 fields followed by an i8 array, with no packed layout.
    HeaderAndBytes { words: [u64; 3], bytes: Bytes },
    /// Any other initializer, including pointers or unresolved expressions.
    Other,
}

/// A global carrying an explicit section or a declaration without storage.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Global {
    pub name: Vec<u8>,
    pub section: Vec<u8>,
    pub constant: bool,
    pub declaration: bool,
    pub link_once_odr: bool,
    pub hidden: bool,
    pub externally_initialized: bool,
    pub alignment: u32,
    pub address_space: u32,
    pub initializer: Option<Initializer>,
}

/// A function with an explicitly assigned section.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Function {
    pub name: Vec<u8>,
    pub section: Vec<u8>,
    pub declaration: bool,
}

/// An inline assembler call/invoke/callbr observed in a function body.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InlineAsm {
    pub function: Vec<u8>,
    pub signature: String,
    pub template: Vec<u8>,
    pub constraints: Vec<u8>,
    pub side_effects: bool,
    pub aligned_stack: bool,
    pub can_unwind: bool,
    pub intel_dialect: bool,
}

/// Explicit section facts in a verified LLVM module. All data is owned by Rust.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Inventory {
    pub triple: Option<String>,
    pub data_layout: String,
    pub globals: Vec<Global>,
    pub functions: Vec<Function>,
    pub module_asm: Vec<u8>,
    pub inline_asm: Vec<InlineAsm>,
    pub aliases: Vec<Vec<u8>>,
    pub indirect_functions: Vec<Vec<u8>>,
}

pub(crate) struct PrintedHeader {
    pub name: Vec<u8>,
    pub section: Vec<u8>,
}

pub(crate) struct PrintedSections {
    pub globals: Vec<PrintedHeader>,
    pub functions: Vec<PrintedHeader>,
}

// LLVM's module printer emits global and function headers in their respective
// list orders. The C API traversal checks each name and both lengths before
// publishing records, so a printer format change cannot misassociate sections.
pub(crate) fn printed_sections(text: &str) -> Result<PrintedSections, String> {
    let mut globals = Vec::new();
    let mut functions = Vec::new();
    for line in text.lines() {
        let is_global = line.starts_with('@');
        let is_function = line.starts_with("define ") || line.starts_with("declare ");
        if !is_global && !is_function {
            continue;
        }
        let bytes = line.as_bytes();
        let mut index = 0;
        while index < bytes.len() {
            if bytes[index] == b'"' {
                skip_quoted(bytes, &mut index, false)?;
            } else if bytes[index] == b'@' {
                break;
            } else {
                index += 1;
            }
        }
        index += 1;
        if index >= bytes.len() {
            return Err("missing LLVM header symbol".into());
        }
        let name = if bytes[index] == b'"' {
            skip_quoted(bytes, &mut index, true)?
        } else {
            let start = index;
            while bytes
                .get(index)
                .is_some_and(|b| b.is_ascii_alphanumeric() || b"._$-".contains(b))
            {
                index += 1;
            }
            bytes[start..index].to_vec()
        };
        if is_global {
            let kind = line[index..]
                .split_whitespace()
                .find(|word| matches!(*word, "constant" | "global" | "alias" | "ifunc"))
                .ok_or("unknown LLVM global header")?;
            if matches!(kind, "alias" | "ifunc") {
                continue;
            }
        }
        let header = PrintedHeader {
            name,
            section: section_attribute(line)?.unwrap_or_default(),
        };
        if is_global {
            globals.push(header);
        } else {
            functions.push(header);
        }
    }
    Ok(PrintedSections { globals, functions })
}

fn skip_quoted(bytes: &[u8], index: &mut usize, capture: bool) -> Result<Vec<u8>, String> {
    let mut result = Vec::new();
    *index += 1;
    while let Some(&byte) = bytes.get(*index) {
        *index += 1;
        match byte {
            b'"' => return Ok(result),
            b'\\' if bytes.get(*index) == Some(&b'\\') => {
                // LLVM's canonical printer uses two backslashes for this byte,
                // while other escaped bytes use two hexadecimal digits.
                if capture {
                    result.push(b'\\');
                }
                *index += 1;
            }
            b'\\' => {
                let hex = bytes.get(*index..*index + 2).ok_or("short LLVM escape")?;
                let hex = std::str::from_utf8(hex).map_err(|_| "invalid LLVM escape")?;
                let byte = u8::from_str_radix(hex, 16)
                    .map_err(|_| format!("invalid LLVM escape at byte {}", *index - 1))?;
                if capture {
                    result.push(byte);
                }
                *index += 2;
            }
            _ if capture => result.push(byte),
            _ => {}
        }
    }
    Err("unterminated LLVM string".into())
}

// LLVMGetSection has no length parameter. Recover the printed attribute so an
// embedded NUL cannot silently turn an unknown section into a known prefix.
pub(crate) fn section_attribute(text: &str) -> Result<Option<Vec<u8>>, String> {
    let bytes = text.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        match bytes[index] {
            b';' => {
                while index < bytes.len() && bytes[index] != b'\n' {
                    index += 1;
                }
            }
            b'"' => {
                skip_quoted(bytes, &mut index, false)?;
            }
            byte if byte.is_ascii_alphabetic() || b"@%$_!".contains(&byte) => {
                let start = index;
                index += 1;
                while index < bytes.len()
                    && (bytes[index].is_ascii_alphanumeric() || b"._$-:".contains(&bytes[index]))
                {
                    index += 1;
                }
                if &bytes[start..index] == b"section" {
                    while bytes.get(index).is_some_and(u8::is_ascii_whitespace) {
                        index += 1;
                    }
                    if bytes.get(index) != Some(&b'"') {
                        return Err("invalid section attribute".into());
                    }
                    return skip_quoted(bytes, &mut index, true).map(Some);
                }
            }
            _ => index += 1,
        }
    }
    Ok(None)
}
