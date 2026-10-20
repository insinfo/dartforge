#![allow(dead_code)]
mod proof {
 struct Abi { result: String, parameters: Vec<String> }
pub(super) fn quoted_end(text: &[u8], start: usize) -> Option<usize> {
    let mut index = start + 1;
    while index < text.len() {
        match text[index] {
            b'"' => return Some(index + 1),
            b'\\' => {
                if !text
                    .get(index + 1..index + 3)?
                    .iter()
                    .all(u8::is_ascii_hexdigit)
                {
                    return None;
                }
                index += 3;
            }
            _ => index += 1,
        }
    }
    None
}

fn arguments(text: &str) -> Option<(Vec<String>, usize)> {
    let bytes = text.as_bytes();
    let mut stack = Vec::new();
    let mut output = Vec::new();
    let mut start = 0;
    let mut index = 0;
    while index < bytes.len() {
        match bytes[index] {
            b'"' => {
                index = quoted_end(bytes, index)?;
                continue;
            }
            b'(' => stack.push(b')'),
            b'[' => stack.push(b']'),
            b'{' => stack.push(b'}'),
            b'<' => stack.push(b'>'),
            b')' if stack.is_empty() => {
                let value = text[start..index].trim();
                if value.is_empty() && !output.is_empty() {
                    return None;
                }
                if !value.is_empty() {
                    output.push(value.to_string());
                }
                return Some((output, index));
            }
            byte @ (b')' | b']' | b'}' | b'>') => {
                if stack.pop() != Some(byte) {
                    return None;
                }
            }
            b',' if stack.is_empty() => {
                let value = text[start..index].trim();
                if value.is_empty() {
                    return None;
                }
                output.push(value.to_string());
                start = index + 1;
            }
            b'\\' => return None,
            _ => {}
        }
        index += 1;
    }
    None
}

fn uncomment(text: &str) -> Option<&str> {
    let bytes = text.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        match bytes[index] {
            b'"' => index = quoted_end(bytes, index)?,
            b';' => return Some(&text[..index]),
            _ => index += 1,
        }
    }
    Some(text)
}

fn signature(text: &str) -> Option<(String, Abi, Vec<String>)> {
    let (prefix, after) = text.split_once('@')?;
    let result = prefix.split_whitespace().last()?;
    if !primitive(result) { return None }
    // Calling conventions, return attributes and constant-expression callees
    // need their own ABI rules; this first inventory does not guess them.
    let prefix_words: Vec<_> = prefix.split_whitespace().collect();
    if prefix_words[..prefix_words.len().checked_sub(1)?].iter().any(|word| !matches!(*word,
        "define" | "declare" | "call" | "internal" | "private" | "dso_local" | "dso_preemptable"
        | "linkonce_odr" | "linkonce" | "weak" | "weak_odr" | "available_externally" | "hidden" | "protected")) { return None }
    let (symbol, tail) = after.split_once('(')?;
    if symbol.is_empty() || !symbol.bytes().all(|b| b.is_ascii_alphanumeric() || b"_$.-".contains(&b)) { return None }
    let (values, end) = arguments(tail)?;
    if prefix.starts_with("call ") && !tail[end + 1..].trim().is_empty() { return None }
    let parameters = values.iter().map(|arg| {
        let mut words = arg.split_whitespace();
        let ty = words.next()?;
        if let Some(value) = words.next() {
            let operand = value.starts_with('%') || value.starts_with('@')
                || value.as_bytes().first().is_some_and(|byte| byte.is_ascii_digit() || *byte == b'-')
                || matches!(value,"null"|"undef"|"poison"|"true"|"false"|"zeroinitializer"|"getelementptr"|"ptrtoint"|"inttoptr"|"bitcast"|"add"|"sub"|"and"|"or"|"xor"|"shl"|"lshr"|"ashr"|"select"|"icmp"|"blockaddress");
            if !operand { return None }
        }
        (primitive(ty) && ty != "void").then(|| ty.to_string())
    }).collect::<Option<Vec<_>>>()?;
    Some((symbol.into(), Abi { result: result.into(), parameters }, values))
}
fn primitive(token: &str) -> bool {
    matches!(token, "void" | "i1" | "i8" | "i16" | "i32" | "i64" | "float" | "double" | "ptr")
}
mod table_getters {
pub(super) fn canonical_symbol(raw: &str) -> Option<String> {
    let mut bytes = Vec::new();
    let mut index = 0;
    while index < raw.len() {
        if raw.as_bytes()[index] == b'\\' {
            let value = raw.get(index + 1..index + 3)?;
            bytes.push(u8::from_str_radix(value, 16).ok()?);
            index += 3;
        } else {
            bytes.push(raw.as_bytes()[index]);
            index += 1;
        }
    }
    String::from_utf8(bytes).ok()
}

}
#[path="/Volumes/ssd_dock/MyRustProjects/dartforge/crates/emit_native/src/pruning/allocation_inventory/closure/read_only.rs"] mod read_only;
#[path="/Volumes/ssd_dock/MyRustProjects/dartforge/crates/emit_native/src/pruning/allocation_inventory/closure/stack_storage.rs"] mod stack_storage;
 pub fn inspect(text: &str) -> (usize,usize,usize,usize) {
 let mut lines=text.lines();let(mut functions,mut with_alloca,mut stores,mut geps)=(0,0,0,0);
 while let Some(header)=lines.next() {
  if !header.starts_with("define ") || !header.ends_with('{') {continue;}
  let mut body=Vec::new();for line in lines.by_ref(){if line.trim()=="}" {break;}body.push(line);}
  let Some((_,_,parameters))=signature(header) else {continue;};functions+=1;
  if body.iter().any(|line|line.contains("alloca")){with_alloca+=1;}
  for index in stack_storage::observe(&body,&parameters){
   let line=uncomment(body[index]).unwrap().trim();
   if line.starts_with("store "){stores+=1;}else{geps+=1;}
  }
 }
 (functions,with_alloca,stores,geps)
 }
}
fn main(){for path in std::env::args().skip(1){let text=std::fs::read_to_string(&path).unwrap();let(a,b,c,d)=proof::inspect(&text);println!("{a} {b} {c} {d} {path}");}}
