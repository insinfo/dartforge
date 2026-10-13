// Native runtime: the VM's `dart:io` — directories (`runtime/bin/directory.cc`,
// `directory_linux.cc`…): the `_Directory` natives, the synchronous listing
// (`Directory_FillWithDirectoryListing`) and the asynchronous one, which the
// IOService advances in batches (`io_service.rs`).

/// `Directory::ExistsResult`.
const DIRECTORY_NOT_FOUND: i64 = 0;
const DIRECTORY_EXISTS: i64 = 1;

/// `Directory::Exists`: exists, does not exist, or error — no permission or a
/// system failure, where it is not known (the VM's `UNKNOWN`); the other errors
/// (does not exist, not a directory, link loop, long name) are "does not exist".
fn directory_exists(p: &std::path::Path) -> IoResult<bool> {
    match std::fs::metadata(p) {
        Ok(m) => Ok(m.is_dir()),
        Err(e) => match e.kind() {
            std::io::ErrorKind::PermissionDenied | std::io::ErrorKind::OutOfMemory => Err(OsError::of(&e)),
            _ => Ok(false),
        },
    }
}

/// `Directory::Create`: a directory that already exists is a success.
fn create_directory(p: &std::path::Path) -> IoResult<()> {
    match std::fs::create_dir(p) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
            if directory_exists(p)? {
                Ok(())
            } else {
                Err(OsError::of(&e))
            }
        }
        Err(e) => Err(OsError::of(&e)),
    }
}

/// `Directory::CreateTemp`: the prefix followed by six random uppercase letters,
/// until a free name.
fn create_temp_directory(prefix: &[u8]) -> IoResult<Vec<u8>> {
    loop {
        let mut name = prefix.to_vec();
        let mut random = entropy();
        for _ in 0..6 {
            name.push(b'A' + (random % 26) as u8);
            random /= 26;
        }
        match std::fs::create_dir(path_from_bytes(&name)) {
            Ok(()) => return Ok(name),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(OsError::of(&e)),
        }
    }
}

/// `Directory::SystemTemp`: `TMPDIR`, `TMP` or `/tmp`, without the trailing slash
/// (Unix); `GetTempPath` without the trailing slash (Windows).
fn system_temp_directory() -> Vec<u8> {
    #[cfg(unix)]
    let mut b = {
        use std::os::unix::ffi::OsStrExt;
        std::env::var_os("TMPDIR")
            .or_else(|| std::env::var_os("TMP"))
            .map_or_else(|| b"/tmp".to_vec(), |v| v.as_bytes().to_vec())
    };
    #[cfg(windows)]
    let mut b = path_bytes_of(&std::env::temp_dir());
    if b.len() > 1 && matches!(b.last(), Some(b'/') | Some(b'\\')) {
        b.pop();
    }
    b
}

/// `Directory::Delete`. Without `recursive`: a link to a directory is deleted as a
/// link, the rest with `rmdir`. With `recursive`: the VM's `DeleteRecursively` —
/// whatever is not a directory (links included) is deleted, and directories are
/// emptied first.
fn delete_directory(p: &std::path::Path, recursive: bool) -> IoResult<()> {
    if !recursive {
        if path_type(p, false) == TYPE_LINK && path_type(p, true) == TYPE_DIRECTORY {
            #[cfg(windows)]
            return Ok(std::fs::remove_dir(p)?);
            #[cfg(unix)]
            return Ok(std::fs::remove_file(p)?);
        }
        return Ok(std::fs::remove_dir(p)?);
    }
    delete_recursively(p)
}

fn delete_recursively(p: &std::path::Path) -> IoResult<()> {
    let m = std::fs::symlink_metadata(p)?;
    if !m.is_dir() {
        #[cfg(windows)]
        if m.file_type().is_symlink() && std::fs::metadata(p).is_ok_and(|d| d.is_dir()) {
            return Ok(std::fs::remove_dir(p)?);
        }
        return Ok(std::fs::remove_file(p)?);
    }
    for input in std::fs::read_dir(p)? {
        delete_recursively(&input?.path())?;
    }
    Ok(std::fs::remove_dir(p)?)
}

/// `Directory::Rename`: the source must be a directory.
fn rename_directory(of: &std::path::Path, to: &std::path::Path) -> IoResult<()> {
    if !directory_exists(of)? {
        return Err(OsError::from_code(os_code::NOT_A_DIRECTORY));
    }
    Ok(std::fs::rename(of, to)?)
}

/// `Directory::Current`.
fn current_directory() -> IoResult<Vec<u8>> {
    Ok(path_bytes_of(&std::env::current_dir()?))
}

// ---------------------------------------------------------------------------
// The listing (the VM's `DirectoryListing`/`DirectoryListingEntry`).

/// The VM's `ListType`.
const LIST_FILE: i64 = 0;
const LIST_DIRECTORY: i64 = 1;
const LIST_LINK: i64 = 2;
const LIST_ERROR: i64 = 3;
const LIST_END: i64 = 4;

/// An entry produced by the listing.
enum ListingEntry {
    /// File, directory or link (`ListType`) and the path in bytes.
    Item(i64, Vec<u8>),
    /// The error and the path where it happened (the VM's `CurrentPath`).
    Error(OsError, Vec<u8>),
    End,
}

/// The identity of a followed link (device and node), so as not to enter a link
/// cycle (the VM's `LinkList`).
type NodeIdentity = (u64, u64);

#[cfg(unix)]
fn node_identity(m: &std::fs::Metadata) -> NodeIdentity {
    use std::os::unix::fs::MetadataExt;
    (m.dev(), m.ino())
}
#[cfg(windows)]
fn node_identity(_m: &std::fs::Metadata) -> NodeIdentity {
    // On Windows, `std` does not expose the stable file index; cycles of directory
    // links are detected through the canonical path.
    (0, 0)
}

/// A stack level: the open directory, the prefix of the entries' paths (with the
/// trailing separator) and the links followed so far.
struct ListingLevel {
    reader: Option<std::fs::ReadDir>,
    prefix: Vec<u8>,
    links: Vec<NodeIdentity>,
}

/// A listing in progress (`DirectoryListing`).
struct Listing {
    stack: Vec<ListingLevel>,
    recursive: bool,
    follow_links_flag: bool,
}

impl Listing {
    /// `file_path` already has the trailing separator (`_ensureTrailingPathSeparators`).
    fn new_area(file_path: Vec<u8>, recursive: bool, follow_links_flag: bool) -> Listing {
        Listing {
            stack: vec![ListingLevel { reader: None, prefix: file_path, links: Vec::new() }],
            recursive,
            follow_links_flag,
        }
    }

    fn empty(&self) -> bool {
        self.stack.is_empty()
    }

    /// The next entry (the VM's `ListNext` + `DirectoryListingEntry::Next`): a
    /// recursive directory pushes its level after being delivered; at the end of a
    /// level, the one above continues.
    fn next_item(&mut self) -> ListingEntry {
        loop {
            let Some(level) = self.stack.last_mut() else {
                return ListingEntry::End;
            };
            if level.reader.is_none() {
                match std::fs::read_dir(path_from_bytes(&level.prefix)) {
                    Ok(r) => level.reader = Some(r),
                    Err(e) => {
                        // The error's path is the directory's: the root's as it
                        // came; a subdirectory's without the separator that the
                        // VM only adds after opening it.
                        let mut file_path = level.prefix.clone();
                        if self.stack.len() > 1 {
                            file_path.truncate(file_path.len() - PATH_SEPARATOR.len());
                        }
                        self.stack.pop();
                        return ListingEntry::Error(OsError::of(&e), file_path);
                    }
                }
            }
            let prefix = level.prefix.clone();
            let input = level.reader.as_mut().expect("opened above").next();
            let input = match input {
                None => {
                    self.stack.pop();
                    continue;
                }
                Some(Err(e)) => {
                    self.stack.pop();
                    return ListingEntry::Error(OsError::of(&e), prefix);
                }
                Some(Ok(e)) => e,
            };
            let mut file_path = prefix;
            file_path.extend_from_slice(&path_bytes_of(std::path::Path::new(&input.file_name())));
            let ty = match self.classify(&input, &file_path) {
                Ok(t) => t,
                Err(e) => return ListingEntry::Error(e, file_path),
            };
            if ty == LIST_DIRECTORY && self.recursive {
                let level = self.stack.last().expect("current level");
                let mut links = level.links.clone();
                if let Ok(m) = std::fs::symlink_metadata(path_from_bytes(&file_path)) {
                    if m.file_type().is_symlink() {
                        if let Ok(target) = std::fs::metadata(path_from_bytes(&file_path)) {
                            links.push(node_identity(&target));
                        }
                    }
                }
                let mut sub = file_path.clone();
                sub.extend_from_slice(PATH_SEPARATOR.as_bytes());
                self.stack.push(ListingLevel { reader: None, prefix: sub, links });
            }
            return ListingEntry::Item(ty, file_path);
        }
    }

    /// The type of an entry: without following links, a link is a link; when
    /// following, the target's type — a broken or cyclic link stays a link.
    fn classify(&self, e: &std::fs::DirEntry, file_path: &[u8]) -> IoResult<i64> {
        let t = e.file_type()?;
        if t.is_dir() {
            return Ok(LIST_DIRECTORY);
        }
        if !t.is_symlink() {
            return Ok(LIST_FILE);
        }
        if !self.follow_links_flag {
            return Ok(LIST_LINK);
        }
        let p = path_from_bytes(file_path);
        let Ok(target) = std::fs::metadata(&p) else {
            return Ok(LIST_LINK);
        };
        if target.is_dir() {
            #[cfg(unix)]
            {
                let id = node_identity(&target);
                if self.stack.last().is_some_and(|n| n.links.contains(&id)) {
                    return Ok(LIST_LINK);
                }
            }
            #[cfg(windows)]
            {
                // A link to an ancestor of the listing is a cycle.
                if let Ok(c) = std::fs::canonicalize(&p) {
                    if self.stack.iter().any(|n| std::fs::canonicalize(path_from_bytes(&n.prefix)).is_ok_and(|a| a == c)) {
                        return Ok(LIST_LINK);
                    }
                }
            }
            return Ok(LIST_DIRECTORY);
        }
        Ok(LIST_FILE)
    }

    /// Closes everything (`PopAll`).
    fn stop(&mut self) {
        self.stack.clear();
    }
}

/// The platform's path separator (`File::PathSeparator`).
#[cfg(unix)]
const PATH_SEPARATOR: &str = "/";
#[cfg(windows)]
const PATH_SEPARATOR: &str = "\\";

// ---------------------------------------------------------------------------
// The `_Directory` natives.

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_Directory_Current(_ns: i64) -> i64 {
    dart_or_error(current_directory(), |b| dart_string_from_bytes(&b))
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_Directory_SetCurrent(_ns: i64, raw: i64) -> i64 {
    dart_true_or_error(std::env::set_current_dir(raw_path(raw)).map_err(OsError::from))
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_Directory_Exists(_ns: i64, raw: i64) -> i64 {
    dart_or_error(directory_exists(&raw_path(raw)), |e| {
        dart_int(if e { DIRECTORY_EXISTS } else { DIRECTORY_NOT_FOUND })
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_Directory_Create(_ns: i64, raw: i64) -> i64 {
    dart_true_or_error(create_directory(&raw_path(raw)))
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_Directory_SystemTemp(_ns: i64) -> i64 {
    dart_string_from_bytes(&system_temp_directory())
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_Directory_CreateTemp(_ns: i64, raw: i64) -> i64 {
    dart_or_error(create_temp_directory(&path_bytes(raw)), |b| dart_string_from_bytes(&b))
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_Directory_Delete(_ns: i64, raw: i64, recursive: u8) -> i64 {
    dart_true_or_error(delete_directory(&raw_path(raw), recursive != 0))
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_Directory_Rename(_ns: i64, raw: i64, new: i64) -> i64 {
    dart_true_or_error(rename_directory(&raw_path(raw), &path_text(new)))
}

/// `Directory_FillWithDirectoryListing`: appends to `list` one
/// `File`/`Directory`/`Link` per entry (`fromRawPath`); the first error is thrown
/// as a `FileSystemException` ("Directory listing failed").
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_Directory_FillWithDirectoryListing(
    _ns: i64,
    list: i64,
    raw: i64,
    recursive: u8,
    follow_links_flag: u8,
) {
    let (Some(input), Some(error)) = (helper("_dartforgeListingEntry"), helper("_dartforgeListingError")) else {
        panic!("compiler bug: dart:io without the overlay's listing functions");
    };
    // SAFETY: registered by the overlay's `dart:io` library with these
    // signatures.
    let input = { let dart_target: usize = input; move |a0: i64, a1: i64, a2: i64| { dart_v3(dart_target, a0, a1, a2) } };
    let error = { let dart_target: usize = error; move |a0: i64, a1: i64| -> i64 { dart_r2(dart_target, a0, a1) } };
    let mut listing = Listing::new_area(path_bytes(raw), recursive != 0, follow_links_flag != 0);
    loop {
        match listing.next_item() {
            ListingEntry::End => return,
            ListingEntry::Item(ty, file_path) => {
                let bytes = dart_bytes(file_path);
                with_roots(&[bytes], || input(list, ty, bytes));
                if dartforge_exception_pending() != 0 {
                    return;
                }
            }
            ListingEntry::Error(e, file_path) => {
                let os = e.to_dart();
                let text = with_roots(&[os], || dart_string_from_bytes(&file_path));
                let exception = with_roots(&[os, text], || error(os, text));
                if dartforge_exception_pending() == 0 {
                    with_roots(&[exception], || dartforge_exception_throw(exception, 3));
                }
                return;
            }
        }
    }
}

// ---------------------------------------------------------------------------
// The asynchronous listing: the native object that the IOService advances.

/// An asynchronous listing (`AsyncDirectoryListing`), shared between the Dart
/// object and the IOService threads through reference counting.
struct AsyncListing {
    listing: std::sync::Mutex<Listing>,
}

impl AsyncListing {
    fn new_area(l: Listing) -> i64 {
        std::sync::Arc::into_raw(std::sync::Arc::new(AsyncListing { listing: std::sync::Mutex::new(l) })) as i64
    }

    fn retain_ref(p: i64) {
        // SAFETY: `p` is a live pointer from `new_area`.
        unsafe { std::sync::Arc::increment_strong_count(p as *const AsyncListing) };
    }

    fn free(p: i64) {
        // SAFETY: `p` is a live pointer from `new_area`, with this reference.
        unsafe { std::sync::Arc::decrement_strong_count(p as *const AsyncListing) };
    }

    /// Takes the reference of `p` (the one of the IOService request).
    ///
    /// # Safety
    /// `p` is a pointer from `new_area` whose reference passes to the caller.
    unsafe fn take(p: i64) -> std::sync::Arc<AsyncListing> {
        // SAFETY: guaranteed by the caller.
        unsafe { std::sync::Arc::from_raw(p as *const AsyncListing) }
    }

    /// Up to `limit` positions of the `ListNext` answer: pairs `[type, file_path]`
    /// (the path in bytes; `null` at the end) and, on an error,
    /// `[kListError, [kListError, file_path, error]]`.
    fn batch(&self, limit: usize) -> Vec<Portable> {
        let mut l = self.listing.lock().unwrap_or_else(|e| e.into_inner());
        let mut out = Vec::new();
        if l.empty() {
            return out;
        }
        while out.len() < limit {
            match l.next_item() {
                ListingEntry::Item(ty, file_path) => {
                    out.push(Portable::Int(ty));
                    out.push(Portable::Bytes(file_path));
                }
                ListingEntry::Error(e, file_path) => {
                    let file_path = String::from_utf8_lossy(&file_path).into_owned();
                    out.push(Portable::Int(LIST_ERROR));
                    out.push(Portable::List(vec![Portable::Int(LIST_ERROR), Portable::Str(file_path), e.to_response()]));
                }
                ListingEntry::End => {
                    out.push(Portable::Int(LIST_END));
                    out.push(Portable::Null);
                    break;
                }
            }
        }
        out
    }
}

/// The finalizer of the Dart object that owns a listing.
fn free_object_listing(pair: usize) {
    AsyncListing::free(pair as i64);
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_Directory_SetAsyncDirectoryListerPointer(this: i64, p: i64) {
    attach_finalizer(this, free_object_listing, p as usize);
    write_native_field(this, p);
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_Directory_GetAsyncDirectoryListerPointer(this: i64) -> i64 {
    let p = native_field(this);
    if p != 0 {
        AsyncListing::retain_ref(p);
    }
    p
}
