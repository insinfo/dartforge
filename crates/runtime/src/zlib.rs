// Native runtime: the `dart:io` zlib filters (`ZLibEncoder`, `ZLibDecoder`,
// `GZipCodec`, `RawZLibFilter`) — the `Filter_*` natives of the VM's
// `filter_patch.dart` (`runtime/bin/filter.cc`), on top of `flate2` with the
// classic zlib (the VM's), compiled from source and linked statically into the
// runtime: the same compressed output as `dart run`, with no zlib on the machine.
//
// The VM protocol: `process(data, start, end)` hands input to the filter;
// `processed(flush:, end:)` returns the available output, or `null` when there
// is no more — the caller repeats until `null`. Compression uses the synchronous
// flush (`Z_SYNC_FLUSH`) on `flush` and closes the stream on `end`.
// Decompression detects zlib or gzip from the header (the VM's
// `windowBits + 32`); `raw` is raw deflate on both sides.
//
// The filter lives in the object's native field (`NativeFieldWrapperClass1`),
// as the `Box` of a `ZlibFilter`, released by the object's finalizer.

use std::io::Write as _;

/// Compression, in the requested format.
enum Compressor {
    Zlib(flate2::write::ZlibEncoder<Vec<u8>>),
    Gzip(flate2::write::GzEncoder<Vec<u8>>),
    Raw(flate2::write::DeflateEncoder<Vec<u8>>),
}

/// Decompression; the format of a non-raw stream comes from the header.
enum Decompressor {
    /// Still without the first two bytes (zlib or gzip?).
    Detecting(Vec<u8>),
    Zlib(flate2::write::ZlibDecoder<Vec<u8>>),
    Gzip(flate2::write::MultiGzDecoder<Vec<u8>>),
    Raw(flate2::write::DeflateDecoder<Vec<u8>>),
}

enum ZlibFilter {
    ZlibCompression(Option<Compressor>),
    Decompression(Decompressor),
}

impl Compressor {
    fn out(&mut self) -> &mut Vec<u8> {
        match self {
            Compressor::Zlib(c) => c.get_mut(),
            Compressor::Gzip(c) => c.get_mut(),
            Compressor::Raw(c) => c.get_mut(),
        }
    }
    fn write_out(&mut self, b: &[u8]) -> std::io::Result<()> {
        match self {
            Compressor::Zlib(c) => c.write_all(b),
            Compressor::Gzip(c) => c.write_all(b),
            Compressor::Raw(c) => c.write_all(b),
        }
    }
    fn flush_out(&mut self) -> std::io::Result<()> {
        match self {
            Compressor::Zlib(c) => c.flush(),
            Compressor::Gzip(c) => c.flush(),
            Compressor::Raw(c) => c.flush(),
        }
    }
    fn terminate(self) -> std::io::Result<Vec<u8>> {
        match self {
            Compressor::Zlib(c) => c.finish(),
            Compressor::Gzip(c) => c.finish(),
            Compressor::Raw(c) => c.finish(),
        }
    }
}

impl Decompressor {
    fn write_out(&mut self, b: &[u8]) -> std::io::Result<()> {
        if let Decompressor::Detecting(pending) = self {
            pending.extend_from_slice(b);
            if pending.len() < 2 {
                return Ok(());
            }
            let pending = std::mem::take(pending);
            *self = if pending[0] == 0x1f && pending[1] == 0x8b {
                Decompressor::Gzip(flate2::write::MultiGzDecoder::new(Vec::new()))
            } else {
                Decompressor::Zlib(flate2::write::ZlibDecoder::new(Vec::new()))
            };
            return self.write_out(&pending);
        }
        match self {
            Decompressor::Zlib(d) => d.write_all(b),
            Decompressor::Gzip(d) => d.write_all(b),
            Decompressor::Raw(d) => d.write_all(b),
            Decompressor::Detecting(_) => unreachable!(),
        }
    }
    fn flush_out(&mut self) -> std::io::Result<()> {
        match self {
            Decompressor::Zlib(d) => d.flush(),
            Decompressor::Gzip(d) => d.flush(),
            Decompressor::Raw(d) => d.flush(),
            Decompressor::Detecting(_) => Ok(()),
        }
    }
    fn take_exit(&mut self) -> Vec<u8> {
        match self {
            Decompressor::Zlib(d) => std::mem::take(d.get_mut()),
            Decompressor::Gzip(d) => std::mem::take(d.get_mut()),
            Decompressor::Raw(d) => std::mem::take(d.get_mut()),
            Decompressor::Detecting(_) => Vec::new(),
        }
    }
}

fn zlib_filter(obj: i64) -> Option<&'static mut ZlibFilter> {
    let p = native_field(obj);
    if p == 0 {
        return None;
    }
    // SAFETY: the field holds the `Box` from `install_zlib_filter`, used only by
    // the owning isolate's thread and alive until the object is collected.
    Some(unsafe { &mut *(p as *mut ZlibFilter) })
}

fn free_zlib_filter(p: usize) {
    // SAFETY: the filter's `Box`, released once by the finalizer.
    unsafe { drop(Box::from_raw(p as *mut ZlibFilter)) };
}

fn install_zlib_filter(this: i64, f: ZlibFilter) {
    let p = Box::into_raw(Box::new(f)) as i64;
    attach_finalizer(this, free_zlib_filter, p as usize);
    write_native_field(this, p);
}

/// The `ZLibOption` level (-1 = zlib's default, 6) for `flate2`.
fn zlib_level(level: i64) -> flate2::Compression {
    if (0..=9).contains(&level) { flate2::Compression::new(level as u32) } else { flate2::Compression::default() }
}

/// The `ZLibOption.dictionary` dictionary is not supported by the Rust backend:
/// `UnsupportedError`, like a feature the platform does not have.
fn reject_dictionary(dictionary: i64) -> bool {
    if dictionary == 0 {
        return false;
    }
    let msg = allocate_str("ZLibOption.dictionary is not supported by dartforge");
    let e = with_roots(&[msg], || dartforge_unsupported_error_new(msg));
    with_roots(&[e], || dartforge_exception_throw(e, 3));
    true
}

/// `Filter_CreateZLibDeflate(gzip, level, windowBits, memLevel, strategy,
/// dictionary, raw)`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_Filter_CreateZLibDeflate(
    this: i64,
    gzip: u8,
    level: i64,
    _window_bits: i64,
    _mem_level: i64,
    _strategy: i64,
    dictionary: i64,
    raw_value: u8,
) {
    if reject_dictionary(dictionary) {
        return;
    }
    let n = zlib_level(level);
    let c = if raw_value != 0 {
        Compressor::Raw(flate2::write::DeflateEncoder::new(Vec::new(), n))
    } else if gzip != 0 {
        Compressor::Gzip(flate2::write::GzEncoder::new(Vec::new(), n))
    } else {
        Compressor::Zlib(flate2::write::ZlibEncoder::new(Vec::new(), n))
    };
    install_zlib_filter(this, ZlibFilter::ZlibCompression(Some(c)));
}

/// `Filter_CreateZLibInflate(gzip, windowBits, dictionary, raw)`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_Filter_CreateZLibInflate(this: i64, _gzip: u8, _window_bits: i64, dictionary: i64, raw_value: u8) {
    if reject_dictionary(dictionary) {
        return;
    }
    let d = if raw_value != 0 {
        Decompressor::Raw(flate2::write::DeflateDecoder::new(Vec::new()))
    } else {
        Decompressor::Detecting(Vec::new())
    };
    install_zlib_filter(this, ZlibFilter::Decompression(d));
}

/// The bytes of a `List<int>` (a typed list, or a list of integers truncated to
/// 8 bits, like the VM's `Dart_ListGetAsBytes`).
fn bytes_from_int_list(h: i64) -> Vec<u8> {
    if let Some(b) = typed_list_bytes(h) {
        return b;
    }
    HEAP.with(|heap| bytes_from_value_list(&heap.borrow(), h))
}

fn throw_invalid_data() {
    let e = allocate_format_exception("Filter error, bad data");
    with_roots(&[e], || dartforge_exception_throw(e, 3));
}

/// `Filter_Process(data, start, end)`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_Filter_Process(this: i64, data: i64, start: i64, end: i64) {
    let Some(f) = zlib_filter(this) else { return throw_internal_error("zlib filter closed") };
    let bytes = bytes_from_int_list(data);
    let end = (end.max(0) as usize).min(bytes.len());
    let start = (start.max(0) as usize).min(end);
    let r = match f {
        ZlibFilter::ZlibCompression(Some(c)) => c.write_out(&bytes[start..end]),
        ZlibFilter::ZlibCompression(None) => Err(std::io::Error::other("fechado")),
        ZlibFilter::Decompression(d) => d.write_out(&bytes[start..end]),
    };
    if r.is_err() {
        throw_invalid_data();
    }
}

/// `Filter_Processed({flush, end})`: the available output, or `null`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_Filter_Processed(this: i64, flush_out: u8, terminate: u8) -> i64 {
    let Some(f) = zlib_filter(this) else { return 0 };
    let out = match f {
        ZlibFilter::ZlibCompression(c) => {
            if terminate != 0 {
                match c.take().map(Compressor::terminate) {
                    Some(Ok(v)) => v,
                    Some(Err(_)) => return { throw_invalid_data(); 0 },
                    None => Vec::new(),
                }
            } else {
                let Some(c) = c else { return 0 };
                if flush_out != 0 && c.flush_out().is_err() {
                    throw_invalid_data();
                    return 0;
                }
                std::mem::take(c.out())
            }
        }
        ZlibFilter::Decompression(d) => {
            if (flush_out != 0 || terminate != 0) && d.flush_out().is_err() {
                throw_invalid_data();
                return 0;
            }
            d.take_exit()
        }
    };
    if out.is_empty() { 0 } else { dart_bytes(out) }
}
