//! The engine frame of each function of a linked WebAssembly module
//! (docs/PLANO-WASM-MVP.md, W4 "StackOverflowError capturável").
//!
//! The frames of the module live on the engine's own stack, not in the linear
//! memory, so the linear stack check cannot see a recursion run out of it. The
//! prologue of every function that calls (`LlvmEmitter::emit_stack_check`)
//! debits its frame from a budget the host gives (`dartforge_engine_budget`)
//! and the returns give it back; below zero the function throws
//! `StackOverflowError` before the engine traps. The size of a frame depends on
//! the function's locals, which only exist after the link: the emitter writes
//! [`FRAME_COST_MARK`] as an `i32.const` (an `asm` the optimizer cannot fold)
//! and [`patch_frame_costs`] rewrites each one with the cost of the function it
//! landed in, in the same five bytes.
//!
//! The cost is that of V8's baseline compiler (Liftoff), the largest frame of
//! the supported engines: every parameter and local in an 8-byte slot plus the
//! fixed part ([`FRAME_BASE`]). Measured in Node 24: a recursion with 9 locals
//! takes 127 bytes per frame, one with 19 parameters and locals 207.

/// The constant of the prologue that [`patch_frame_costs`] replaces: its signed
/// LEB128 has five bytes, as large as an `i32` can take.
pub const FRAME_COST_MARK: i32 = 755_939_893;

/// The fixed part of a frame, in bytes (return address, frame pointer, the
/// instance and the spills of the operand stack around calls).
pub const FRAME_BASE: u32 = 64;

/// The bytes of each parameter or local of the frame.
pub const FRAME_SLOT: u32 = 8;

/// The five bytes of `value` as a signed LEB128 (padded: WebAssembly accepts
/// any encoding up to the five bytes of an `i32`).
fn leb5(value: i32) -> [u8; 5] {
    let v = value as u32;
    [
        (v & 0x7f) as u8 | 0x80,
        ((v >> 7) & 0x7f) as u8 | 0x80,
        ((v >> 14) & 0x7f) as u8 | 0x80,
        ((v >> 21) & 0x7f) as u8 | 0x80,
        ((v >> 28) & 0x7f) as u8,
    ]
}

struct Reader<'a> {
    bytes: &'a [u8],
    at: usize,
}

impl Reader<'_> {
    fn byte(&mut self) -> Result<u8, String> {
        let b = *self
            .bytes
            .get(self.at)
            .ok_or("the module ends inside a section")?;
        self.at += 1;
        Ok(b)
    }

    fn leb(&mut self) -> Result<u32, String> {
        let mut value = 0u32;
        let mut shift = 0;
        loop {
            let b = self.byte()?;
            if shift > 28 {
                return Err("LEB128 longer than 32 bits".to_string());
            }
            value |= u32::from(b & 0x7f) << shift;
            if b & 0x80 == 0 {
                return Ok(value);
            }
            shift += 7;
        }
    }

    fn skip(&mut self, n: usize) -> Result<(), String> {
        self.at = self
            .at
            .checked_add(n)
            .filter(|&e| e <= self.bytes.len())
            .ok_or("the module ends inside a section")?;
        Ok(())
    }

    /// Skips a limits (`flags`, `min`, optional `max`).
    fn limits(&mut self) -> Result<(), String> {
        let flags = self.byte()?;
        self.leb()?;
        if flags & 1 != 0 {
            self.leb()?;
        }
        Ok(())
    }
}

/// The module with each [`FRAME_COST_MARK`] of a function body replaced by
/// that function's frame cost; `None` when the module has no mark (the module
/// is not a Dart program, or none of its functions calls). Every other byte
/// stays.
///
/// # Errors
/// A module that does not parse, a function whose frame does not fit an
/// `i32`.
pub fn patch_frame_costs(bytes: &[u8]) -> Result<Option<Vec<u8>>, String> {
    if bytes.len() < 8 || &bytes[..4] != b"\0asm" {
        return Err("not a WebAssembly module".to_string());
    }
    let mut mark = vec![0x41u8];
    mark.extend(leb5(FRAME_COST_MARK));
    let mut out = bytes.to_vec();
    let mut params_of_type: Vec<u32> = Vec::new();
    let mut function_types: Vec<u32> = Vec::new();
    let mut patched = false;
    let mut r = Reader { bytes, at: 8 };
    while r.at < bytes.len() {
        let id = r.byte()?;
        let size = r.leb()? as usize;
        let start = r.at;
        let end = start
            .checked_add(size)
            .filter(|&e| e <= bytes.len())
            .ok_or("a section runs past the module")?;
        match id {
            // Types: the number of parameters of each.
            1 => {
                for _ in 0..r.leb()? {
                    if r.byte()? != 0x60 {
                        return Err("a type that is not a function type".to_string());
                    }
                    let params = r.leb()?;
                    r.skip(params as usize)?;
                    let results = r.leb()?;
                    r.skip(results as usize)?;
                    params_of_type.push(params);
                }
            }
            // Imports: skipped (the code section only has the defined functions).
            2 => {
                for _ in 0..r.leb()? {
                    let module = r.leb()? as usize;
                    r.skip(module)?;
                    let name = r.leb()? as usize;
                    r.skip(name)?;
                    match r.byte()? {
                        0 => {
                            r.leb()?;
                        }
                        1 => {
                            r.byte()?;
                            r.limits()?;
                        }
                        2 => r.limits()?,
                        3 => r.skip(2)?,
                        k => return Err(format!("an import of unknown kind {k}")),
                    }
                }
            }
            // Functions: the type of each defined function.
            3 => {
                for _ in 0..r.leb()? {
                    function_types.push(r.leb()?);
                }
            }
            // Code: the locals of each body, and its marks.
            10 => {
                let count = r.leb()? as usize;
                if count != function_types.len() {
                    return Err("the code and function sections disagree".to_string());
                }
                for &ty in function_types.iter() {
                    let body_size = r.leb()? as usize;
                    let body_start = r.at;
                    let body_end = body_start
                        .checked_add(body_size)
                        .filter(|&e| e <= end)
                        .ok_or("a function body runs past the code section")?;
                    let mut locals = 0u64;
                    for _ in 0..r.leb()? {
                        locals += u64::from(r.leb()?);
                        r.byte()?;
                    }
                    let params = params_of_type
                        .get(ty as usize)
                        .copied()
                        .ok_or("a function of an unknown type")?;
                    let instructions = r.at;
                    let body = &bytes[instructions..body_end];
                    let mut at = 0;
                    let mut cost = None;
                    while let Some(i) = body[at..]
                        .windows(mark.len())
                        .position(|w| w == mark.as_slice())
                    {
                        let cost = match cost {
                            Some(c) => c,
                            None => {
                                let c = u64::from(FRAME_BASE)
                                    + u64::from(FRAME_SLOT) * (u64::from(params) + locals);
                                let c = i32::try_from(c).map_err(|_| {
                                    format!("a frame of {c} bytes does not fit an i32")
                                })?;
                                cost = Some(c);
                                c
                            }
                        };
                        let p = instructions + at + i + 1;
                        out[p..p + 5].copy_from_slice(&leb5(cost));
                        patched = true;
                        at += i + mark.len();
                    }
                    r.at = body_end;
                }
            }
            _ => {}
        }
        r.at = end;
    }
    Ok(patched.then_some(out))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn leb(mut v: u32) -> Vec<u8> {
        let mut out = Vec::new();
        loop {
            let b = (v & 0x7f) as u8;
            v >>= 7;
            if v == 0 {
                out.push(b);
                return out;
            }
            out.push(b | 0x80);
        }
    }

    fn section(id: u8, body: &[u8]) -> Vec<u8> {
        let mut s = vec![id];
        s.extend(leb(body.len() as u32));
        s.extend(body);
        s
    }

    /// A module with one imported function and two bodies: the first with two
    /// parameters, three locals and two marks, the second with none.
    fn module() -> Vec<u8> {
        let mut m = b"\0asm\x01\0\0\0".to_vec();
        // (i32, i32) -> i32 and () -> ().
        m.extend(section(1, &[2, 0x60, 2, 0x7f, 0x7f, 1, 0x7f, 0x60, 0, 0]));
        m.extend(section(2, &[1, 1, b'm', 1, b'f', 0, 1]));
        m.extend(section(3, &[2, 0, 1]));
        let mut first = vec![2, 1, 0x7f, 2, 0x7e];
        for _ in 0..2 {
            first.push(0x41);
            first.extend(leb5(FRAME_COST_MARK));
            first.push(0x1a);
        }
        first.extend([0x41, 0, 0x0b]);
        let second = vec![0, 0x0b];
        let mut code = vec![2];
        code.extend(leb(first.len() as u32));
        code.extend(&first);
        code.extend(leb(second.len() as u32));
        code.extend(&second);
        m.extend(section(10, &code));
        m
    }

    /// Decodes a signed LEB128 `i32.const` immediate.
    fn sleb(bytes: &[u8]) -> i32 {
        let mut v = 0i64;
        let mut shift = 0;
        for &b in bytes {
            v |= i64::from(b & 0x7f) << shift;
            shift += 7;
            if b & 0x80 == 0 {
                if b & 0x40 != 0 && shift < 64 {
                    v |= -1i64 << shift;
                }
                break;
            }
        }
        v as i32
    }

    #[test]
    fn the_mark_is_five_bytes_and_decodes() {
        let m = leb5(FRAME_COST_MARK);
        assert_eq!(sleb(&m), FRAME_COST_MARK);
        // The minimal encoding of the mark (what the assembler writes) is the
        // same five bytes.
        assert_eq!(m[4] & 0x80, 0);
        assert!(m[..4].iter().all(|b| b & 0x80 != 0));
        assert_eq!(sleb(&leb5(136)), 136);
    }

    #[test]
    fn each_mark_becomes_the_frame_of_its_function() {
        let original = module();
        let patched = patch_frame_costs(&original).unwrap().unwrap();
        assert_eq!(patched.len(), original.len());
        let cost = (FRAME_BASE + FRAME_SLOT * (2 + 3)) as i32;
        let mut found = 0;
        let mut at = 0;
        while let Some(i) = patched[at..]
            .windows(6)
            .position(|w| w[0] == 0x41 && w[5] & 0x80 == 0 && w[1..5].iter().all(|b| b & 0x80 != 0))
        {
            assert_eq!(sleb(&patched[at + i + 1..at + i + 6]), cost);
            found += 1;
            at += i + 6;
        }
        assert_eq!(found, 2);
        // Only the immediates changed.
        let diff = original
            .iter()
            .zip(&patched)
            .filter(|(a, b)| a != b)
            .count();
        assert!(diff <= 10, "{diff}");
    }

    #[test]
    fn a_module_without_marks_is_left_alone() {
        let mut m = b"\0asm\x01\0\0\0".to_vec();
        m.extend(section(1, &[1, 0x60, 0, 0]));
        m.extend(section(3, &[1, 0]));
        m.extend(section(10, &[1, 2, 0, 0x0b]));
        assert_eq!(patch_frame_costs(&m).unwrap(), None);
        assert!(patch_frame_costs(b"not wasm").is_err());
    }
}
