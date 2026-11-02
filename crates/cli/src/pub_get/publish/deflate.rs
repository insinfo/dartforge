//! The gzip encoder of the Dart VM (`gzip.encoder` of `dart:io`), which is
//! Chromium's zlib at level 6 (window bits 15, memory level 8, default
//! strategy).
//!
//! `dart pub publish` shows the compressed size of the archive truncated to
//! whole KB, and uploads the archive itself, so the bytes have to be the
//! same as the VM's. Chromium's zlib differs from the classic one only in
//! how strings are hashed (`insert_string` of
//! `contrib/optimizations/insert_string.h`: a multiplicative hash of four
//! bytes over a 15-bit table, which also changes when `fill_window` inserts
//! strings); the rest is zlib 1.3's `deflate_slow` and `trees.c`. This is a
//! port of exactly that path (revision `108fa50c` of
//! `chromium/src/third_party/zlib`, the one of the 3.6.2 SDK), checked byte
//! for byte against the VM. Only what level 6 reaches is ported: no stored,
//! fast, RLE or Huffman-only strategies, no dictionaries and no flushes other
//! than the final one.

const MIN_MATCH: u32 = 3;
const MAX_MATCH: u32 = 258;
const MIN_LOOKAHEAD: u32 = MAX_MATCH + MIN_MATCH + 1;
const W_SIZE: u32 = 1 << 15;
const W_MASK: u32 = W_SIZE - 1;
const WINDOW_SIZE: usize = 2 * W_SIZE as usize;
/// `WINDOW_PADDING` of Chromium: the window is `2 * (w_size + 8)` bytes.
const WINDOW_ALLOCATION: usize = 2 * (W_SIZE as usize + 8);
const MAX_DIST: u32 = W_SIZE - MIN_LOOKAHEAD;
const WIN_INIT: usize = MAX_MATCH as usize;
const HASH_BITS: u32 = 15;
const HASH_SIZE: usize = 1 << HASH_BITS;
const HASH_MASK: u32 = (1 << HASH_BITS) - 1;
const LIT_BUFSIZE: usize = 1 << (8 + 6);
const SYM_END: usize = LIT_BUFSIZE - 1;
const TOO_FAR: u32 = 4096;
const NIL: u32 = 0;

// Level 6 of `configuration_table`.
const GOOD_MATCH: u32 = 8;
const MAX_LAZY: u32 = 16;
const NICE_MATCH: u32 = 128;
const MAX_CHAIN: u32 = 128;

const LENGTH_CODES: usize = 29;
const LITERALS: usize = 256;
const L_CODES: usize = LITERALS + 1 + LENGTH_CODES;
const D_CODES: usize = 30;
const BL_CODES: usize = 19;
const HEAP_SIZE: usize = 2 * L_CODES + 1;
const MAX_BITS: usize = 15;
const MAX_BL_BITS: usize = 7;
const END_BLOCK: usize = 256;
const REP_3_6: usize = 16;
const REPZ_3_10: usize = 17;
const REPZ_11_138: usize = 18;
const BUF_SIZE: i32 = 16;

const STORED_BLOCK: u32 = 0;
const STATIC_TREES: u32 = 1;
const DYN_TREES: u32 = 2;

const EXTRA_LBITS: [u32; LENGTH_CODES] = [
    0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2, 3, 3, 3, 3, 4, 4, 4, 4, 5, 5, 5, 5, 0,
];
const EXTRA_DBITS: [u32; D_CODES] = [
    0, 0, 0, 0, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7, 8, 8, 9, 9, 10, 10, 11, 11, 12, 12, 13,
    13,
];
const EXTRA_BLBITS: [u32; BL_CODES] = [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 2, 3, 7];
const BL_ORDER: [usize; BL_CODES] = [
    16, 17, 18, 0, 8, 7, 9, 6, 10, 5, 11, 4, 12, 3, 13, 2, 14, 1, 15,
];

/// The gzip header's OS byte (`OS_CODE` of `zutil.h`).
const OS_CODE: u8 = if cfg!(target_vendor = "apple") {
    19
} else if cfg!(windows) {
    10
} else {
    3
};

/// The static tables of `tr_static_init`.
struct Tables {
    length_code: [u8; (MAX_MATCH - MIN_MATCH + 1) as usize],
    dist_code: [u8; 512],
    base_length: [u32; LENGTH_CODES],
    base_dist: [u32; D_CODES],
    static_ltree: [(u16, u16); L_CODES + 2],
    static_dtree: [(u16, u16); D_CODES],
}

fn bi_reverse(mut code: u32, mut len: u32) -> u32 {
    let mut res = 0;
    loop {
        res |= code & 1;
        code >>= 1;
        res <<= 1;
        len -= 1;
        if len == 0 {
            break;
        }
    }
    res >> 1
}

/// `gen_codes`: the codes of `lens[..=max_code]` into `codes`.
fn gen_codes(lens: &[u16], codes: &mut [u16], max_code: usize, bl_count: &[u16; MAX_BITS + 1]) {
    let mut next_code = [0u16; MAX_BITS + 1];
    let mut code: u32 = 0;
    for bits in 1..=MAX_BITS {
        code = (code + u32::from(bl_count[bits - 1])) << 1;
        next_code[bits] = code as u16;
    }
    for n in 0..=max_code {
        let len = lens[n] as usize;
        if len == 0 {
            continue;
        }
        codes[n] = bi_reverse(u32::from(next_code[len]), len as u32) as u16;
        next_code[len] = next_code[len].wrapping_add(1);
    }
}

impl Tables {
    fn new() -> Self {
        let mut t = Tables {
            length_code: [0; (MAX_MATCH - MIN_MATCH + 1) as usize],
            dist_code: [0; 512],
            base_length: [0; LENGTH_CODES],
            base_dist: [0; D_CODES],
            static_ltree: [(0, 0); L_CODES + 2],
            static_dtree: [(0, 0); D_CODES],
        };
        let mut length = 0usize;
        let mut code = 0;
        while code < LENGTH_CODES - 1 {
            t.base_length[code] = length as u32;
            for _ in 0..(1 << EXTRA_LBITS[code]) {
                t.length_code[length] = code as u8;
                length += 1;
            }
            code += 1;
        }
        t.length_code[length - 1] = code as u8;
        let mut dist = 0usize;
        code = 0;
        while code < 16 {
            t.base_dist[code] = dist as u32;
            for _ in 0..(1 << EXTRA_DBITS[code]) {
                t.dist_code[dist] = code as u8;
                dist += 1;
            }
            code += 1;
        }
        dist >>= 7;
        while code < D_CODES {
            t.base_dist[code] = (dist << 7) as u32;
            for _ in 0..(1 << (EXTRA_DBITS[code] - 7)) {
                t.dist_code[256 + dist] = code as u8;
                dist += 1;
            }
            code += 1;
        }
        let mut bl_count = [0u16; MAX_BITS + 1];
        let mut lens = [0u16; L_CODES + 2];
        for (n, len) in lens.iter_mut().enumerate() {
            *len = match n {
                0..=143 => 8,
                144..=255 => 9,
                256..=279 => 7,
                _ => 8,
            };
            bl_count[*len as usize] += 1;
        }
        let mut codes = [0u16; L_CODES + 2];
        gen_codes(&lens, &mut codes, L_CODES + 1, &bl_count);
        for n in 0..L_CODES + 2 {
            t.static_ltree[n] = (codes[n], lens[n]);
        }
        for n in 0..D_CODES {
            t.static_dtree[n] = (bi_reverse(n as u32, 5) as u16, 5);
        }
        t
    }

    fn d_code(&self, dist: u32) -> usize {
        if dist < 256 {
            self.dist_code[dist as usize] as usize
        } else {
            self.dist_code[256 + (dist >> 7) as usize] as usize
        }
    }
}

/// A dynamic tree: `ct_data` is a union of `Freq`/`Code` and `Dad`/`Len`,
/// and zlib relies on the overlap, so the two halves are kept as two arrays.
struct Tree {
    /// `Freq`, later `Code`.
    fc: Vec<u16>,
    /// `Dad`, later `Len`.
    dl: Vec<u16>,
    max_code: i32,
    /// `static_tree_desc`: the static tree of this kind, if any.
    has_static: bool,
    extra_base: usize,
    elems: usize,
    max_length: usize,
}

impl Tree {
    fn new(
        size: usize,
        has_static: bool,
        extra_base: usize,
        elems: usize,
        max_length: usize,
    ) -> Self {
        Tree {
            fc: vec![0; size],
            dl: vec![0; size],
            max_code: 0,
            has_static,
            extra_base,
            elems,
            max_length,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Literal,
    Distance,
    BitLength,
}

/// The heap and counters of `build_tree`.
struct Heap {
    heap: [i32; 2 * L_CODES + 1],
    heap_len: usize,
    heap_max: usize,
    depth: [u8; 2 * L_CODES + 1],
    bl_count: [u16; MAX_BITS + 1],
    opt_len: u64,
    static_len: u64,
}

impl Heap {
    fn smaller(tree: &Tree, depth: &[u8], n: i32, m: i32) -> bool {
        let (n, m) = (n as usize, m as usize);
        tree.fc[n] < tree.fc[m] || (tree.fc[n] == tree.fc[m] && depth[n] <= depth[m])
    }

    fn pqdownheap(&mut self, tree: &Tree, mut k: usize) {
        let v = self.heap[k];
        let mut j = k << 1;
        while j <= self.heap_len {
            if j < self.heap_len && Self::smaller(tree, &self.depth, self.heap[j + 1], self.heap[j])
            {
                j += 1;
            }
            if Self::smaller(tree, &self.depth, v, self.heap[j]) {
                break;
            }
            self.heap[k] = self.heap[j];
            k = j;
            j <<= 1;
        }
        self.heap[k] = v;
    }
}

/// A raw deflate stream with a gzip wrapper, written into a growing buffer
/// (so `avail_out` is never zero and no early return of zlib is reached).
struct Deflater<'a> {
    tables: Tables,
    input: &'a [u8],
    next_in: usize,
    window: Vec<u8>,
    prev: Vec<u16>,
    head: Vec<u16>,
    high_water: usize,
    ins_h: u32,
    strstart: u32,
    block_start: i64,
    lookahead: u32,
    insert: u32,
    match_length: u32,
    prev_length: u32,
    match_available: bool,
    match_start: u32,
    prev_match: u32,
    ltree: Tree,
    dtree: Tree,
    bltree: Tree,
    heap: Heap,
    d_buf: Vec<u16>,
    l_buf: Vec<u8>,
    sym_next: usize,
    bi_buf: u16,
    bi_valid: i32,
    out: Vec<u8>,
}

/// Compresses `data` into the bytes of `gzip.encode(data)` of the Dart VM.
pub(crate) fn gzip(data: &[u8]) -> Vec<u8> {
    let mut d = Deflater::new(data);
    d.out
        .extend_from_slice(&[0x1f, 0x8b, 8, 0, 0, 0, 0, 0, 0, OS_CODE]);
    d.deflate_slow_finish();
    let crc = crc32(data);
    d.out.extend_from_slice(&crc.to_le_bytes());
    d.out.extend_from_slice(&(data.len() as u32).to_le_bytes());
    d.out
}

/// The CRC-32 of gzip.
pub(crate) fn crc32(data: &[u8]) -> u32 {
    static TABLE: std::sync::OnceLock<[u32; 256]> = std::sync::OnceLock::new();
    let table = TABLE.get_or_init(|| {
        let mut table = [0u32; 256];
        for (n, entry) in table.iter_mut().enumerate() {
            let mut c = n as u32;
            for _ in 0..8 {
                c = if c & 1 != 0 {
                    0xedb8_8320 ^ (c >> 1)
                } else {
                    c >> 1
                };
            }
            *entry = c;
        }
        table
    });
    let mut crc = !0u32;
    for &byte in data {
        crc = table[((crc ^ u32::from(byte)) & 0xff) as usize] ^ (crc >> 8);
    }
    !crc
}

impl<'a> Deflater<'a> {
    fn new(input: &'a [u8]) -> Self {
        let mut d = Deflater {
            tables: Tables::new(),
            input,
            next_in: 0,
            window: vec![0; WINDOW_ALLOCATION],
            prev: vec![0; W_SIZE as usize],
            head: vec![0; HASH_SIZE],
            high_water: 0,
            ins_h: 0,
            strstart: 0,
            block_start: 0,
            lookahead: 0,
            insert: 0,
            match_length: MIN_MATCH - 1,
            prev_length: MIN_MATCH - 1,
            match_available: false,
            match_start: 0,
            prev_match: 0,
            ltree: Tree::new(HEAP_SIZE, true, LITERALS + 1, L_CODES, MAX_BITS),
            dtree: Tree::new(2 * D_CODES + 1, true, 0, D_CODES, MAX_BITS),
            bltree: Tree::new(2 * BL_CODES + 1, false, 0, BL_CODES, MAX_BL_BITS),
            heap: Heap {
                heap: [0; 2 * L_CODES + 1],
                heap_len: 0,
                heap_max: 0,
                depth: [0; 2 * L_CODES + 1],
                bl_count: [0; MAX_BITS + 1],
                opt_len: 0,
                static_len: 0,
            },
            d_buf: vec![0; LIT_BUFSIZE],
            l_buf: vec![0; LIT_BUFSIZE],
            sym_next: 0,
            bi_buf: 0,
            bi_valid: 0,
            out: Vec::with_capacity(input.len() / 3 + 64),
        };
        d.init_block();
        d
    }

    fn avail_in(&self) -> usize {
        self.input.len() - self.next_in
    }

    /// Chromium's `insert_string`.
    fn insert_string(&mut self, s: u32) -> u32 {
        let i = s as usize;
        let value = u32::from_le_bytes([
            self.window[i],
            self.window[i + 1],
            self.window[i + 2],
            self.window[i + 3],
        ]);
        self.ins_h = (value.wrapping_mul(66521).wrapping_add(66521) >> 16) & HASH_MASK;
        let head = self.head[self.ins_h as usize];
        self.prev[(s & W_MASK) as usize] = head;
        self.head[self.ins_h as usize] = s as u16;
        u32::from(head)
    }

    fn slide_hash(&mut self) {
        for m in self.head.iter_mut() {
            *m = if u32::from(*m) >= W_SIZE {
                (u32::from(*m) - W_SIZE) as u16
            } else {
                NIL as u16
            };
        }
        for m in self.prev.iter_mut() {
            *m = if u32::from(*m) >= W_SIZE {
                (u32::from(*m) - W_SIZE) as u16
            } else {
                NIL as u16
            };
        }
    }

    fn read_buf(&mut self, at: usize, size: usize) -> usize {
        let len = self.avail_in().min(size);
        if len == 0 {
            return 0;
        }
        self.window[at..at + len].copy_from_slice(&self.input[self.next_in..self.next_in + len]);
        self.next_in += len;
        len
    }

    fn fill_window(&mut self) {
        loop {
            let mut more = (WINDOW_SIZE as u64
                - u64::from(self.lookahead)
                - u64::from(self.strstart)) as usize;
            if self.strstart >= W_SIZE + MAX_DIST {
                let w = W_SIZE as usize;
                self.window.copy_within(w..2 * w - more, 0);
                self.match_start = self.match_start.wrapping_sub(W_SIZE);
                self.strstart -= W_SIZE;
                self.block_start -= i64::from(W_SIZE);
                if self.insert > self.strstart {
                    self.insert = self.strstart;
                }
                self.slide_hash();
                more += w;
            }
            if self.avail_in() == 0 {
                break;
            }
            let at = (self.strstart + self.lookahead) as usize;
            let n = self.read_buf(at, more);
            self.lookahead += n as u32;
            // Chromium's hash reads four bytes.
            if self.lookahead + self.insert > MIN_MATCH {
                let mut s = self.strstart - self.insert;
                while self.insert != 0 {
                    self.insert_string(s);
                    s += 1;
                    self.insert -= 1;
                    if self.lookahead + self.insert <= MIN_MATCH {
                        break;
                    }
                }
            }
            if !(self.lookahead < MIN_LOOKAHEAD && self.avail_in() != 0) {
                break;
            }
        }
        if self.high_water < WINDOW_SIZE {
            let curr = (self.strstart + self.lookahead) as usize;
            if self.high_water < curr {
                let init = (WINDOW_SIZE - curr).min(WIN_INIT);
                self.window[curr..curr + init].fill(0);
                self.high_water = curr + init;
            } else if self.high_water < curr + WIN_INIT {
                let init = (curr + WIN_INIT - self.high_water).min(WINDOW_SIZE - self.high_water);
                self.window[self.high_water..self.high_water + init].fill(0);
                self.high_water += init;
            }
        }
    }

    fn longest_match(&mut self, mut cur_match: u32) -> u32 {
        let mut chain_length = MAX_CHAIN;
        let scan = self.strstart as usize;
        let mut best_len = self.prev_length as usize;
        let mut nice_match = NICE_MATCH as usize;
        let limit = if self.strstart > MAX_DIST {
            self.strstart - MAX_DIST
        } else {
            NIL
        };
        let strend = scan + MAX_MATCH as usize;
        let w = &self.window;
        let mut scan_end1 = w[scan + best_len - 1];
        let mut scan_end = w[scan + best_len];
        if self.prev_length >= GOOD_MATCH {
            chain_length >>= 2;
        }
        if nice_match > self.lookahead as usize {
            nice_match = self.lookahead as usize;
        }
        loop {
            let m = cur_match as usize;
            if !(w[m + best_len] != scan_end
                || w[m + best_len - 1] != scan_end1
                || w[m] != w[scan]
                || w[m + 1] != w[scan + 1])
            {
                // scan += 2, match++ (then the loop pre-increments both).
                let mut s = scan + 2;
                let mut mm = m + 2;
                loop {
                    let mut stop = false;
                    for _ in 0..8 {
                        s += 1;
                        mm += 1;
                        if w[s] != w[mm] {
                            stop = true;
                            break;
                        }
                    }
                    if stop || s >= strend {
                        break;
                    }
                }
                let len = MAX_MATCH as usize - (strend - s);
                if len > best_len {
                    self.match_start = cur_match;
                    best_len = len;
                    if len >= nice_match {
                        break;
                    }
                    scan_end1 = w[scan + best_len - 1];
                    scan_end = w[scan + best_len];
                }
            }
            cur_match = u32::from(self.prev[(cur_match & W_MASK) as usize]);
            if cur_match <= limit {
                break;
            }
            chain_length -= 1;
            if chain_length == 0 {
                break;
            }
        }
        if best_len as u32 <= self.lookahead {
            best_len as u32
        } else {
            self.lookahead
        }
    }

    fn tally_lit(&mut self, c: u8) -> bool {
        self.d_buf[self.sym_next] = 0;
        self.l_buf[self.sym_next] = c;
        self.sym_next += 1;
        self.ltree.fc[c as usize] += 1;
        self.sym_next == SYM_END
    }

    fn tally_dist(&mut self, distance: u32, length: u32) -> bool {
        let len = length as u8;
        let dist = distance as u16;
        self.d_buf[self.sym_next] = dist;
        self.l_buf[self.sym_next] = len;
        self.sym_next += 1;
        let dist = dist.wrapping_sub(1);
        let lcode = self.tables.length_code[len as usize] as usize + LITERALS + 1;
        self.ltree.fc[lcode] += 1;
        let dcode = self.tables.d_code(u32::from(dist));
        self.dtree.fc[dcode] += 1;
        self.sym_next == SYM_END
    }

    fn flush_block(&mut self, last: bool) {
        let stored = if self.block_start >= 0 {
            Some(self.block_start as usize)
        } else {
            None
        };
        let stored_len = (i64::from(self.strstart) - self.block_start) as u64;
        self.tr_flush_block(stored, stored_len, last);
        self.block_start = i64::from(self.strstart);
    }

    /// `deflate_slow` with `Z_FINISH` and all the input available.
    fn deflate_slow_finish(&mut self) {
        loop {
            if self.lookahead < MIN_LOOKAHEAD {
                self.fill_window();
                if self.lookahead == 0 {
                    break;
                }
            }
            let mut hash_head = NIL;
            if self.lookahead >= MIN_MATCH {
                hash_head = self.insert_string(self.strstart);
            }
            self.prev_length = self.match_length;
            self.prev_match = self.match_start;
            self.match_length = MIN_MATCH - 1;
            if hash_head != NIL
                && self.prev_length < MAX_LAZY
                && self.strstart - hash_head <= MAX_DIST
            {
                self.match_length = self.longest_match(hash_head);
                if self.match_length <= 5
                    && self.match_length == MIN_MATCH
                    && self.strstart.wrapping_sub(self.match_start) > TOO_FAR
                {
                    self.match_length = MIN_MATCH - 1;
                }
            }
            if self.prev_length >= MIN_MATCH && self.match_length <= self.prev_length {
                let max_insert = self.strstart + self.lookahead - MIN_MATCH;
                let bflush = self.tally_dist(
                    self.strstart.wrapping_sub(1).wrapping_sub(self.prev_match),
                    self.prev_length - MIN_MATCH,
                );
                self.lookahead -= self.prev_length - 1;
                self.prev_length -= 2;
                loop {
                    self.strstart += 1;
                    if self.strstart <= max_insert {
                        self.insert_string(self.strstart);
                    }
                    self.prev_length -= 1;
                    if self.prev_length == 0 {
                        break;
                    }
                }
                self.match_available = false;
                self.match_length = MIN_MATCH - 1;
                self.strstart += 1;
                if bflush {
                    self.flush_block(false);
                }
            } else if self.match_available {
                let c = self.window[(self.strstart - 1) as usize];
                if self.tally_lit(c) {
                    self.flush_block(false);
                }
                self.strstart += 1;
                self.lookahead -= 1;
            } else {
                self.match_available = true;
                self.strstart += 1;
                self.lookahead -= 1;
            }
        }
        if self.match_available {
            let c = self.window[(self.strstart - 1) as usize];
            self.tally_lit(c);
            self.match_available = false;
        }
        self.insert = self.strstart.min(MIN_MATCH - 1);
        self.flush_block(true);
    }

    // ---- trees.c ----

    fn init_block(&mut self) {
        for n in 0..L_CODES {
            self.ltree.fc[n] = 0;
        }
        for n in 0..D_CODES {
            self.dtree.fc[n] = 0;
        }
        for n in 0..BL_CODES {
            self.bltree.fc[n] = 0;
        }
        self.ltree.fc[END_BLOCK] = 1;
        self.heap.opt_len = 0;
        self.heap.static_len = 0;
        self.sym_next = 0;
    }

    fn put_byte(&mut self, b: u8) {
        self.out.push(b);
    }

    fn put_short(&mut self, w: u16) {
        self.out.push((w & 0xff) as u8);
        self.out.push((w >> 8) as u8);
    }

    fn send_bits(&mut self, value: u32, length: u32) {
        // C promotes the 16-bit values to `int` before shifting, so a shift
        // by 16 empties the value instead of wrapping the shift count.
        let len = length as i32;
        let val = u32::from(value as u16);
        if self.bi_valid > BUF_SIZE - len {
            self.bi_buf |= (val << self.bi_valid) as u16;
            self.put_short(self.bi_buf);
            self.bi_buf = (val >> (BUF_SIZE - self.bi_valid)) as u16;
            self.bi_valid += len - BUF_SIZE;
        } else {
            self.bi_buf |= (val << self.bi_valid) as u16;
            self.bi_valid += len;
        }
    }

    fn bi_windup(&mut self) {
        if self.bi_valid > 8 {
            self.put_short(self.bi_buf);
        } else if self.bi_valid > 0 {
            self.put_byte(self.bi_buf as u8);
        }
        self.bi_buf = 0;
        self.bi_valid = 0;
    }

    fn tree_mut(&mut self, kind: Kind) -> &mut Tree {
        match kind {
            Kind::Literal => &mut self.ltree,
            Kind::Distance => &mut self.dtree,
            Kind::BitLength => &mut self.bltree,
        }
    }

    fn static_len_of(&self, kind: Kind, n: usize) -> u64 {
        match kind {
            Kind::Literal => u64::from(self.tables.static_ltree[n].1),
            Kind::Distance => u64::from(self.tables.static_dtree[n].1),
            Kind::BitLength => 0,
        }
    }

    fn extra_bits(kind: Kind, n: usize) -> u32 {
        match kind {
            Kind::Literal => EXTRA_LBITS[n],
            Kind::Distance => EXTRA_DBITS[n],
            Kind::BitLength => EXTRA_BLBITS[n],
        }
    }

    fn gen_bitlen(&mut self, kind: Kind) {
        let static_lens: Vec<u64> = {
            let tree = match kind {
                Kind::Literal => &self.ltree,
                Kind::Distance => &self.dtree,
                Kind::BitLength => &self.bltree,
            };
            (0..tree.elems)
                .map(|n| {
                    if tree.has_static {
                        self.static_len_of(kind, n)
                    } else {
                        0
                    }
                })
                .collect()
        };
        let mut tree = std::mem::replace(self.tree_mut(kind), Tree::new(0, false, 0, 0, 0));
        let h = &mut self.heap;
        let max_code = tree.max_code;
        let base = tree.extra_base;
        let max_length = tree.max_length;
        let mut overflow = 0i32;
        h.bl_count = [0; MAX_BITS + 1];
        let root = h.heap[h.heap_max] as usize;
        tree.dl[root] = 0;
        let mut hi = h.heap_max + 1;
        while hi < HEAP_SIZE {
            let n = h.heap[hi] as usize;
            hi += 1;
            let mut bits = tree.dl[tree.dl[n] as usize] as usize + 1;
            if bits > max_length {
                bits = max_length;
                overflow += 1;
            }
            tree.dl[n] = bits as u16;
            if n as i32 > max_code {
                continue;
            }
            h.bl_count[bits] += 1;
            let mut xbits = 0;
            if n >= base {
                xbits = Self::extra_bits(kind, n - base);
            }
            let f = u64::from(tree.fc[n]);
            h.opt_len = h.opt_len.wrapping_add(f * (bits as u64 + u64::from(xbits)));
            if tree.has_static {
                h.static_len = h
                    .static_len
                    .wrapping_add(f * (static_lens[n] + u64::from(xbits)));
            }
        }
        if overflow != 0 {
            loop {
                let mut bits = max_length - 1;
                while h.bl_count[bits] == 0 {
                    bits -= 1;
                }
                h.bl_count[bits] -= 1;
                h.bl_count[bits + 1] += 2;
                h.bl_count[max_length] -= 1;
                overflow -= 2;
                if overflow <= 0 {
                    break;
                }
            }
            let mut hh = HEAP_SIZE;
            let mut bits = max_length;
            while bits != 0 {
                let mut n = h.bl_count[bits];
                while n != 0 {
                    hh -= 1;
                    let m = h.heap[hh] as usize;
                    if m as i32 > max_code {
                        continue;
                    }
                    if u32::from(tree.dl[m]) != bits as u32 {
                        h.opt_len = h.opt_len.wrapping_add(
                            (bits as u64)
                                .wrapping_sub(u64::from(tree.dl[m]))
                                .wrapping_mul(u64::from(tree.fc[m])),
                        );
                        tree.dl[m] = bits as u16;
                    }
                    n -= 1;
                }
                bits -= 1;
            }
        }
        *self.tree_mut(kind) = tree;
    }

    fn build_tree(&mut self, kind: Kind) {
        let mut tree = std::mem::replace(self.tree_mut(kind), Tree::new(0, false, 0, 0, 0));
        let elems = tree.elems;
        let mut max_code: i32 = -1;
        {
            let h = &mut self.heap;
            h.heap_len = 0;
            h.heap_max = HEAP_SIZE;
            for n in 0..elems {
                if tree.fc[n] != 0 {
                    h.heap_len += 1;
                    h.heap[h.heap_len] = n as i32;
                    max_code = n as i32;
                    h.depth[n] = 0;
                } else {
                    tree.dl[n] = 0;
                }
            }
        }
        while self.heap.heap_len < 2 {
            let node = if max_code < 2 {
                max_code += 1;
                max_code
            } else {
                0
            };
            self.heap.heap_len += 1;
            self.heap.heap[self.heap.heap_len] = node;
            let node = node as usize;
            tree.fc[node] = 1;
            self.heap.depth[node] = 0;
            self.heap.opt_len = self.heap.opt_len.wrapping_sub(1);
            if tree.has_static {
                let sl = self.static_len_of(kind, node);
                self.heap.static_len = self.heap.static_len.wrapping_sub(sl);
            }
        }
        tree.max_code = max_code;
        let h = &mut self.heap;
        let mut n = h.heap_len / 2;
        while n >= 1 {
            h.pqdownheap(&tree, n);
            n -= 1;
        }
        let mut node = elems;
        loop {
            // pqremove
            let n = h.heap[1];
            h.heap[1] = h.heap[h.heap_len];
            h.heap_len -= 1;
            h.pqdownheap(&tree, 1);
            let m = h.heap[1];
            h.heap_max -= 1;
            h.heap[h.heap_max] = n;
            h.heap_max -= 1;
            h.heap[h.heap_max] = m;
            let (nu, mu) = (n as usize, m as usize);
            tree.fc[node] = tree.fc[nu].wrapping_add(tree.fc[mu]);
            h.depth[node] = h.depth[nu].max(h.depth[mu]) + 1;
            tree.dl[nu] = node as u16;
            tree.dl[mu] = node as u16;
            h.heap[1] = node as i32;
            node += 1;
            h.pqdownheap(&tree, 1);
            if h.heap_len < 2 {
                break;
            }
        }
        h.heap_max -= 1;
        h.heap[h.heap_max] = h.heap[1];
        *self.tree_mut(kind) = tree;
        self.gen_bitlen(kind);
        let tree = self.tree_mut(kind);
        let lens = tree.dl.clone();
        let max = tree.max_code as usize;
        let bl_count = self.heap.bl_count;
        let tree = self.tree_mut(kind);
        gen_codes(&lens, &mut tree.fc, max, &bl_count);
    }

    fn scan_tree(&mut self, kind: Kind) {
        let tree = self.tree_mut(kind);
        let max_code = tree.max_code as usize;
        tree.dl[max_code + 1] = 0xffff;
        let lens = tree.dl.clone();
        let mut prevlen: i32 = -1;
        let mut nextlen = i32::from(lens[0]);
        let mut count = 0;
        let (mut max_count, mut min_count) = (7, 4);
        if nextlen == 0 {
            max_count = 138;
            min_count = 3;
        }
        for n in 0..=max_code {
            let curlen = nextlen;
            nextlen = i32::from(lens[n + 1]);
            count += 1;
            if count < max_count && curlen == nextlen {
                continue;
            } else if count < min_count {
                self.bltree.fc[curlen as usize] =
                    self.bltree.fc[curlen as usize].wrapping_add(count as u16);
            } else if curlen != 0 {
                if curlen != prevlen {
                    self.bltree.fc[curlen as usize] += 1;
                }
                self.bltree.fc[REP_3_6] += 1;
            } else if count <= 10 {
                self.bltree.fc[REPZ_3_10] += 1;
            } else {
                self.bltree.fc[REPZ_11_138] += 1;
            }
            count = 0;
            prevlen = curlen;
            if nextlen == 0 {
                max_count = 138;
                min_count = 3;
            } else if curlen == nextlen {
                max_count = 6;
                min_count = 3;
            } else {
                max_count = 7;
                min_count = 4;
            }
        }
    }

    fn send_code(&mut self, c: usize, kind: Kind) {
        let (code, len) = match kind {
            Kind::Literal => (self.ltree.fc[c], self.ltree.dl[c]),
            Kind::Distance => (self.dtree.fc[c], self.dtree.dl[c]),
            Kind::BitLength => (self.bltree.fc[c], self.bltree.dl[c]),
        };
        self.send_bits(u32::from(code), u32::from(len));
    }

    fn send_tree(&mut self, kind: Kind, max_code: usize) {
        let lens = match kind {
            Kind::Literal => self.ltree.dl.clone(),
            _ => self.dtree.dl.clone(),
        };
        let mut prevlen: i32 = -1;
        let mut nextlen = i32::from(lens[0]);
        let mut count: i32 = 0;
        let (mut max_count, mut min_count) = (7, 4);
        if nextlen == 0 {
            max_count = 138;
            min_count = 3;
        }
        for n in 0..=max_code {
            let curlen = nextlen;
            nextlen = i32::from(lens[n + 1]);
            count += 1;
            if count < max_count && curlen == nextlen {
                continue;
            } else if count < min_count {
                loop {
                    self.send_code(curlen as usize, Kind::BitLength);
                    count -= 1;
                    if count == 0 {
                        break;
                    }
                }
            } else if curlen != 0 {
                if curlen != prevlen {
                    self.send_code(curlen as usize, Kind::BitLength);
                    count -= 1;
                }
                self.send_code(REP_3_6, Kind::BitLength);
                self.send_bits((count - 3) as u32, 2);
            } else if count <= 10 {
                self.send_code(REPZ_3_10, Kind::BitLength);
                self.send_bits((count - 3) as u32, 3);
            } else {
                self.send_code(REPZ_11_138, Kind::BitLength);
                self.send_bits((count - 11) as u32, 7);
            }
            count = 0;
            prevlen = curlen;
            if nextlen == 0 {
                max_count = 138;
                min_count = 3;
            } else if curlen == nextlen {
                max_count = 6;
                min_count = 3;
            } else {
                max_count = 7;
                min_count = 4;
            }
        }
    }

    fn build_bl_tree(&mut self) -> usize {
        self.scan_tree(Kind::Literal);
        self.scan_tree(Kind::Distance);
        self.build_tree(Kind::BitLength);
        let mut max_blindex = BL_CODES - 1;
        while max_blindex >= 3 {
            if self.bltree.dl[BL_ORDER[max_blindex]] != 0 {
                break;
            }
            max_blindex -= 1;
        }
        self.heap.opt_len = self
            .heap
            .opt_len
            .wrapping_add(3 * (max_blindex as u64 + 1) + 5 + 5 + 4);
        max_blindex
    }

    fn send_all_trees(&mut self, lcodes: usize, dcodes: usize, blcodes: usize) {
        self.send_bits((lcodes - 257) as u32, 5);
        self.send_bits((dcodes - 1) as u32, 5);
        self.send_bits((blcodes - 4) as u32, 4);
        for &order in BL_ORDER.iter().take(blcodes) {
            let len = self.bltree.dl[order];
            self.send_bits(u32::from(len), 3);
        }
        self.send_tree(Kind::Literal, lcodes - 1);
        self.send_tree(Kind::Distance, dcodes - 1);
    }

    fn stored_block(&mut self, buf: Option<usize>, stored_len: u64, last: bool) {
        self.send_bits((STORED_BLOCK << 1) + u32::from(last), 3);
        self.bi_windup();
        self.put_short(stored_len as u16);
        self.put_short(!(stored_len as u16));
        if stored_len != 0
            && let Some(start) = buf
        {
            let end = start + stored_len as usize;
            let bytes = self.window[start..end].to_vec();
            self.out.extend_from_slice(&bytes);
        }
    }

    fn compress_block(&mut self, dynamic: bool) {
        let mut sx = 0;
        if self.sym_next != 0 {
            loop {
                let mut dist = u32::from(self.d_buf[sx]);
                let mut lc = u32::from(self.l_buf[sx]);
                sx += 1;
                if dist == 0 {
                    self.send_lit(lc as usize, dynamic);
                } else {
                    let code = self.tables.length_code[lc as usize] as usize;
                    self.send_lit(code + LITERALS + 1, dynamic);
                    let extra = EXTRA_LBITS[code];
                    if extra != 0 {
                        lc -= self.tables.base_length[code];
                        self.send_bits(lc, extra);
                    }
                    dist -= 1;
                    let code = self.tables.d_code(dist);
                    if dynamic {
                        self.send_code(code, Kind::Distance);
                    } else {
                        let (c, l) = self.tables.static_dtree[code];
                        self.send_bits(u32::from(c), u32::from(l));
                    }
                    let extra = EXTRA_DBITS[code];
                    if extra != 0 {
                        dist -= self.tables.base_dist[code];
                        self.send_bits(dist, extra);
                    }
                }
                if sx >= self.sym_next {
                    break;
                }
            }
        }
        self.send_lit(END_BLOCK, dynamic);
    }

    fn send_lit(&mut self, c: usize, dynamic: bool) {
        if dynamic {
            self.send_code(c, Kind::Literal);
        } else {
            let (code, len) = self.tables.static_ltree[c];
            self.send_bits(u32::from(code), u32::from(len));
        }
    }

    fn tr_flush_block(&mut self, buf: Option<usize>, stored_len: u64, last: bool) {
        self.build_tree(Kind::Literal);
        self.build_tree(Kind::Distance);
        let max_blindex = self.build_bl_tree();
        let mut opt_lenb = self.heap.opt_len.wrapping_add(3 + 7) >> 3;
        let static_lenb = self.heap.static_len.wrapping_add(3 + 7) >> 3;
        if static_lenb <= opt_lenb {
            opt_lenb = static_lenb;
        }
        if stored_len + 4 <= opt_lenb && buf.is_some() {
            self.stored_block(buf, stored_len, last);
        } else if static_lenb == opt_lenb {
            self.send_bits((STATIC_TREES << 1) + u32::from(last), 3);
            self.compress_block(false);
        } else {
            self.send_bits((DYN_TREES << 1) + u32::from(last), 3);
            let lcodes = self.ltree.max_code as usize + 1;
            let dcodes = self.dtree.max_code as usize + 1;
            self.send_all_trees(lcodes, dcodes, max_blindex + 1);
            self.compress_block(true);
        }
        self.init_block();
        if last {
            self.bi_windup();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crc32_matches_the_standard_check_value() {
        assert_eq!(crc32(b"123456789"), 0xcbf4_3926);
    }

    /// Inflates a raw deflate stream (the gzip body) to check round trips.
    fn gunzip(bytes: &[u8]) -> Vec<u8> {
        use std::io::Read;
        let mut out = Vec::new();
        flate2::read::GzDecoder::new(bytes)
            .read_to_end(&mut out)
            .expect("valid gzip");
        out
    }

    #[test]
    fn round_trips_through_inflate() {
        let mut inputs: Vec<Vec<u8>> = vec![
            Vec::new(),
            b"a".to_vec(),
            b"abcabcabcabcabcabcabc".to_vec(),
            vec![0u8; 200_000],
        ];
        // Pseudo-random text with repetition, larger than two windows.
        let mut state = 12345u32;
        let words = [
            "dart", "pub", "publish", "package", "\n", " ", "lib/", "{", "}",
        ];
        let mut text = Vec::new();
        while text.len() < 300_000 {
            state = state.wrapping_mul(1_103_515_245).wrapping_add(12345);
            text.extend_from_slice(words[(state >> 16) as usize % words.len()].as_bytes());
            if state.is_multiple_of(7) {
                text.push((state >> 8) as u8);
            }
        }
        inputs.push(text);
        let mut noise = Vec::new();
        for _ in 0..100_000 {
            state = state.wrapping_mul(1_103_515_245).wrapping_add(12345);
            noise.push((state >> 16) as u8);
        }
        inputs.push(noise);
        for input in inputs {
            let compressed = gzip(&input);
            assert_eq!(gunzip(&compressed), input);
        }
    }

    #[test]
    fn small_input_matches_the_vm() {
        // `gzip.encode(utf8.encode('hello hello hello'))` on the 3.6.2 VM,
        // without the header (its OS byte depends on the platform).
        let out = gzip(b"hello hello hello");
        assert_eq!(
            &out[10..],
            &[
                0xcb, 0x48, 0xcd, 0xc9, 0xc9, 0x57, 0xc8, 0x40, 0x90, 0x00, 0x80, 0x88, 0xf9, 0xe5,
                0x11, 0x00, 0x00, 0x00
            ]
        );
    }

    /// Compares with a corpus made by the Dart VM: every file `X` of the
    /// directory in `DARTFORGE_GZIP_CORPUS` next to `X.gz`, the output of
    /// `gzip.encode` of the 3.6.2 VM on the same platform.
    #[test]
    #[ignore = "needs DARTFORGE_GZIP_CORPUS made with the Dart VM"]
    fn matches_a_corpus_of_the_vm() {
        let directory = std::env::var_os("DARTFORGE_GZIP_CORPUS")
            .expect("DARTFORGE_GZIP_CORPUS names the corpus directory");
        let mut checked = 0;
        for entry in std::fs::read_dir(directory).expect("corpus directory") {
            let path = entry.expect("corpus entry").path();
            if path.extension().is_some_and(|e| e == "gz") {
                continue;
            }
            let expected = std::fs::read(path.with_extension(match path.extension() {
                Some(e) => format!("{}.gz", e.to_string_lossy()),
                None => "gz".into(),
            }))
            .expect("the VM's output");
            let input = std::fs::read(&path).expect("input");
            assert!(gzip(&input) == expected, "{} differs", path.display());
            checked += 1;
        }
        assert!(checked > 0);
    }
}
