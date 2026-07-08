//! # WASM OPFS — Origin Private File System persistence for AgentDB
//!
//! This module implements persistent browser storage via the
//! [Origin Private File System (OPFS)][opfs-spec] API, a W3C standard available
//! in all modern browsers (Chrome 102+, Firefox 111+, Safari 15.2+).
//!
//! ## Strategy: load-save pattern
//!
//! SQLite's synchronous VFS interface requires synchronous I/O, which is only
//! possible on OPFS from a dedicated `Worker` via `FileSystemSyncAccessHandle`.
//! On the main thread (where wasm-bindgen runs) we can only use async OPFS APIs.
//!
//! We therefore use the **load-save pattern** popularised by sql.js:
//!
//! 1. **On open** — read the entire `.agentdb` file from OPFS as a byte buffer,
//!    deserialize it into a SQLite in-memory database using `sqlite3_deserialize`.
//! 2. **On save** — serialize the in-memory database to bytes via `sqlite3_serialize`
//!    and write the resulting buffer back to the OPFS file.
//!
//! This gives full ACID semantics within a single browser session and durable
//! persistence across page reloads, at the cost of loading the entire DB into
//! memory (appropriate for the agent use-case where DBs are typically < 50 MB).
//!
//! ## Public API
//!
//! | Function | Description |
//! |---|---|
//! | [`open_persistent`] | Load (or create) a named DB from OPFS |
//! | [`save`] | Flush an in-memory DB back to OPFS |
//! | [`delete_persistent`] | Remove an OPFS database file |
//! | [`list_databases`] | List all `.agentdb` files in the OPFS root |
//!
//! [opfs-spec]: https://fs.spec.whatwg.org/#origin-private-file-system

#[cfg(feature = "wasm")]
use wasm_bindgen::prelude::*;

#[cfg(feature = "wasm")]
use wasm_bindgen_futures::JsFuture;

#[cfg(feature = "wasm")]
use js_sys::{Array, Reflect, Uint8Array};

#[cfg(feature = "wasm")]
use web_sys::{FileSystemDirectoryHandle, FileSystemGetFileOptions};

#[cfg(feature = "wasm")]
use crate::wasm::WasmAgentDB;

// ── OPFS helpers ─────────────────────────────────────────────────────────────

/// Obtain the OPFS root `FileSystemDirectoryHandle`.
///
/// Calls `navigator.storage.getDirectory()` and awaits the resulting `Promise`.
/// Returns `Err` if the browser does not support OPFS.
#[cfg(feature = "wasm")]
async fn opfs_root() -> Result<FileSystemDirectoryHandle, JsValue> {
    let window = web_sys::window()
        .ok_or_else(|| JsValue::from_str("OPFS: no global `window` object — are you in a Worker?"))?;

    let navigator = window.navigator();
    let storage = navigator.storage();

    // `storage.get_directory()` returns a `Promise<FileSystemDirectoryHandle>`
    let promise = storage.get_directory();
    let handle = JsFuture::from(promise).await?;

    handle
        .dyn_into::<FileSystemDirectoryHandle>()
        .map_err(|_| JsValue::from_str("OPFS: getDirectory() did not return a FileSystemDirectoryHandle"))
}

/// Build the OPFS filename for a logical database name.
#[cfg(feature = "wasm")]
fn db_filename(name: &str) -> String {
    format!("{}.agentdb", name)
}

/// Read an entire OPFS file as bytes. Returns an empty `Vec` if the file does
/// not exist yet (so that a first-open creates a fresh database).
#[cfg(feature = "wasm")]
async fn opfs_read_bytes(dir: &FileSystemDirectoryHandle, filename: &str) -> Result<Vec<u8>, JsValue> {
    // Attempt to get the file handle without `create: true`.
    // If the file does not exist the browser rejects the promise with a
    // `NotFoundError` — we catch that and return an empty buffer.
    let opts = FileSystemGetFileOptions::new();
    // `create` defaults to false, which is what we want for reading.
    let get_promise = dir.get_file_handle_with_options(filename, &opts);
    let file_handle_val = match JsFuture::from(get_promise).await {
        Ok(v) => v,
        Err(err) => {
            // Treat NotFoundError as "file doesn't exist yet" → empty bytes
            let name_prop = Reflect::get(&err, &JsValue::from_str("name"))
                .unwrap_or_default();
            if name_prop.as_string().as_deref() == Some("NotFoundError") {
                return Ok(Vec::new());
            }
            return Err(err);
        }
    };

    let file_handle = file_handle_val
        .dyn_into::<web_sys::FileSystemFileHandle>()
        .map_err(|_| JsValue::from_str("OPFS: getFileHandle() did not return a FileSystemFileHandle"))?;

    // file_handle.getFile() → Promise<File>
    let file_val = JsFuture::from(file_handle.get_file()).await?;
    let file = file_val
        .dyn_into::<web_sys::File>()
        .map_err(|_| JsValue::from_str("OPFS: getFile() did not return a File"))?;

    // file.arrayBuffer() → Promise<ArrayBuffer>
    let ab_val = JsFuture::from(file.array_buffer()).await?;
    let uint8 = Uint8Array::new(&ab_val);
    Ok(uint8.to_vec())
}

/// Write a byte slice to an OPFS file, replacing its entire contents.
#[cfg(feature = "wasm")]
async fn opfs_write_bytes(
    dir: &FileSystemDirectoryHandle,
    filename: &str,
    data: &[u8],
) -> Result<(), JsValue> {
    // Get-or-create the file handle with `create: true`
    let opts = FileSystemGetFileOptions::new();
    opts.set_create(true);

    let file_handle_val =
        JsFuture::from(dir.get_file_handle_with_options(filename, &opts)).await?;
    let file_handle = file_handle_val
        .dyn_into::<web_sys::FileSystemFileHandle>()
        .map_err(|_| {
            JsValue::from_str(
                "OPFS: getFileHandle(create=true) did not return a FileSystemFileHandle",
            )
        })?;

    // createWritable() → Promise<FileSystemWritableFileStream>
    // The stream gives us atomic "write then swap" semantics: the file is only
    // replaced when we call close().
    let writable_val = JsFuture::from(file_handle.create_writable()).await?;
    let writable = writable_val
        .dyn_into::<web_sys::FileSystemWritableFileStream>()
        .map_err(|_| {
            JsValue::from_str(
                "OPFS: createWritable() did not return a FileSystemWritableFileStream",
            )
        })?;

    // write(u8 slice) — replaces the file contents
    JsFuture::from(writable.write_with_u8_array(data)?).await?;

    // close() — atomically finalises the write and flushes to storage
    JsFuture::from(writable.close()).await?;

    Ok(())
}

// ── SQLite serialize / deserialize via libsqlite3-sys FFI ────────────────────

/// Flags for `sqlite3_deserialize`.
#[cfg(feature = "wasm")]
const SQLITE_DESERIALIZE_FREEONCLOSE: u32 = 0x0001;
#[cfg(feature = "wasm")]
const SQLITE_DESERIALIZE_RESIZEABLE: u32 = 0x0002;

/// Serialise a `rusqlite::Connection` to a raw SQLite database image.
///
/// Uses the low-level `sqlite3_serialize` API (SQLite >= 3.23.0, always
/// present when using the bundled SQLite).
///
/// Returns the database as an owned `Vec<u8>`.
#[cfg(feature = "wasm")]
pub(crate) fn sqlite_serialize(conn: &rusqlite::Connection) -> Result<Vec<u8>, JsValue> {
    use libsqlite3_sys as ffi;
    use std::ffi::CString;

    // SAFETY: We hold a live `rusqlite::Connection` reference, so the internal
    // sqlite3* pointer is valid for the duration of this call.
    // `sqlite3_serialize` with flags=0 returns a `sqlite3_malloc`-allocated
    // buffer that we must copy into an owned Vec and then free.
    unsafe {
        let db_ptr = conn.handle();
        let schema = CString::new("main").expect("CString: no NUL in \"main\"");
        let mut size: ffi::sqlite3_int64 = 0;
        let buf_ptr = ffi::sqlite3_serialize(db_ptr, schema.as_ptr(), &mut size, 0);

        if buf_ptr.is_null() {
            return Err(JsValue::from_str(
                "sqlite3_serialize returned NULL (out of memory?)",
            ));
        }

        // Copy the bytes out before freeing the SQLite-allocated buffer
        let len = size as usize;
        let bytes = std::slice::from_raw_parts(buf_ptr, len).to_vec();

        // Free the buffer allocated by SQLite
        ffi::sqlite3_free(buf_ptr as *mut std::ffi::c_void);

        Ok(bytes)
    }
}

/// Deserialise a raw SQLite database image into an open in-memory connection.
///
/// The `bytes` slice must contain a valid SQLite database image (produced by
/// [`sqlite_serialize`] or any other means).  The connection is opened as
/// `":memory:"` before calling this function — the deserialization replaces
/// its content.
///
/// Uses `sqlite3_deserialize` with `SQLITE_DESERIALIZE_RESIZEABLE |
/// SQLITE_DESERIALIZE_FREEONCLOSE`. We copy `bytes` into a `sqlite3_malloc`
/// buffer so that SQLite owns the memory for the lifetime of the connection.
#[cfg(feature = "wasm")]
pub(crate) fn sqlite_deserialize(
    conn: &rusqlite::Connection,
    bytes: &[u8],
) -> Result<(), JsValue> {
    use libsqlite3_sys as ffi;
    use std::ffi::CString;

    if bytes.is_empty() {
        // Nothing to restore — the connection is already a fresh empty DB.
        return Ok(());
    }

    // SAFETY: We allocate a buffer with sqlite3_malloc, copy the bytes in,
    // and pass ownership to SQLite via SQLITE_DESERIALIZE_FREEONCLOSE.
    // The connection pointer is valid for the lifetime of `conn`.
    unsafe {
        let db_ptr = conn.handle();
        let schema = CString::new("main").unwrap();
        let len = bytes.len() as ffi::sqlite3_int64;

        // Allocate a SQLite-owned buffer
        let buf = ffi::sqlite3_malloc64(bytes.len() as u64) as *mut u8;
        if buf.is_null() {
            return Err(JsValue::from_str("sqlite3_malloc64 failed (out of memory)"));
        }
        std::ptr::copy_nonoverlapping(bytes.as_ptr(), buf, bytes.len());

        let rc = ffi::sqlite3_deserialize(
            db_ptr,
            schema.as_ptr(),
            buf,
            len,
            len,
            (SQLITE_DESERIALIZE_FREEONCLOSE | SQLITE_DESERIALIZE_RESIZEABLE) as u32,
        );

        if rc != ffi::SQLITE_OK {
            return Err(JsValue::from_str(&format!(
                "sqlite3_deserialize failed with code {}",
                rc
            )));
        }

        Ok(())
    }
}

// ── Public async API ─────────────────────────────────────────────────────────

/// Open (or create) a **persistent** AgentDB database stored in the browser's
/// Origin Private File System.
///
/// `name` is the logical database name; the actual OPFS filename will be
/// `<name>.agentdb`.
///
/// # How it works
///
/// 1. Obtains the OPFS root via `navigator.storage.getDirectory()`.
/// 2. Reads `<name>.agentdb` into memory (returns empty bytes on first open).
/// 3. Opens a fresh in-memory SQLite connection and runs schema migrations.
/// 4. If bytes were loaded, deserialises them into the connection via
///    `sqlite3_deserialize`, restoring the full database state.
/// 5. Returns a [`WasmAgentDB`] wrapping the connection.
///
/// # Errors
///
/// - If the browser does not support OPFS.
/// - If OPFS I/O fails (quota exceeded, permission denied, …).
/// - If the stored bytes are not a valid SQLite image.
#[cfg(feature = "wasm")]
#[wasm_bindgen]
pub async fn open_persistent(name: &str) -> Result<WasmAgentDB, JsValue> {
    // 1. Get the OPFS root directory
    let root = opfs_root().await?;

    // 2. Read existing bytes (empty vec if the file doesn't exist yet)
    let filename = db_filename(name);
    let bytes = opfs_read_bytes(&root, &filename).await?;

    // 3. Open a fresh in-memory AgentDB (runs schema bootstrap automatically)
    let db = WasmAgentDB::open_memory()?;

    // 4. Restore bytes if we loaded any
    if !bytes.is_empty() {
        db.deserialize_bytes(&bytes)?;
    }

    Ok(db)
}

/// Persist a [`WasmAgentDB`]'s current state to OPFS.
///
/// Serialises the in-memory database to a byte buffer via `sqlite3_serialize`
/// and writes it atomically to `<name>.agentdb` in the OPFS root using
/// `FileSystemWritableFileStream` (which provides atomic replace semantics).
///
/// Call this after every batch of writes that should survive a page reload.
///
/// # Errors
///
/// - If OPFS is unavailable.
/// - If serialisation or the OPFS write fails.
#[cfg(feature = "wasm")]
#[wasm_bindgen]
pub async fn save(db: &WasmAgentDB, name: &str) -> Result<(), JsValue> {
    let root = opfs_root().await?;
    let filename = db_filename(name);
    let bytes = db.serialize_bytes()?;
    opfs_write_bytes(&root, &filename, &bytes).await
}

/// Delete a persistent database from OPFS.
///
/// Removes `<name>.agentdb` from the OPFS root. Silently succeeds if the file
/// does not exist.
///
/// # Errors
///
/// - If OPFS is unavailable.
/// - If the browser rejects the removal for reasons other than `NotFoundError`.
#[cfg(feature = "wasm")]
#[wasm_bindgen]
pub async fn delete_persistent(name: &str) -> Result<(), JsValue> {
    let root = opfs_root().await?;
    let filename = db_filename(name);

    let promise = root.remove_entry(&filename);
    match JsFuture::from(promise).await {
        Ok(_) => Ok(()),
        Err(err) => {
            // NotFoundError is fine — the file is already gone
            let name_prop = Reflect::get(&err, &JsValue::from_str("name"))
                .unwrap_or_default();
            if name_prop.as_string().as_deref() == Some("NotFoundError") {
                Ok(())
            } else {
                Err(err)
            }
        }
    }
}

/// List all AgentDB databases stored in OPFS.
///
/// Returns the **logical names** (i.e. the stem without `.agentdb`) of every
/// `.agentdb` file found in the OPFS root directory.
///
/// ```js
/// import init, { list_databases } from './pkg/agentdb.js';
/// await init();
/// const names = await list_databases(); // e.g. ["myagent", "testdb"]
/// ```
///
/// # Errors
///
/// Returns an error if OPFS is unavailable or iteration fails.
#[cfg(feature = "wasm")]
#[wasm_bindgen]
pub async fn list_databases() -> Result<Vec<String>, JsValue> {
    let root = opfs_root().await?;

    // `FileSystemDirectoryHandle` implements the async iterable protocol.
    // We call the `entries()` iterator method via js_sys.
    //
    // entries() returns an async iterator of [name, handle] pairs.
    // We drive it manually using the .next() → Promise<{done, value}> protocol.
    let entries_iter = js_sys::Reflect::get(&root, &JsValue::from_str("entries"))
        .map_err(|_| JsValue::from_str("OPFS: FileSystemDirectoryHandle.entries() not available"))?;

    let entries_fn = entries_iter
        .dyn_ref::<js_sys::Function>()
        .ok_or_else(|| JsValue::from_str("OPFS: entries is not a function"))?;

    let iterator = entries_fn.call0(&root)?;

    let next_fn_val = Reflect::get(&iterator, &JsValue::from_str("next"))?;
    let next_fn = next_fn_val
        .dyn_ref::<js_sys::Function>()
        .ok_or_else(|| JsValue::from_str("OPFS: iterator.next is not a function"))?;

    let mut names = Vec::new();

    loop {
        // Call next() — returns a Promise<{done: bool, value: [name, handle]}>
        let next_promise_val = next_fn.call0(&iterator)?;
        let next_result = if let Some(promise) = next_promise_val.dyn_ref::<js_sys::Promise>() {
            JsFuture::from(promise.clone()).await?
        } else {
            next_promise_val
        };

        let done = Reflect::get(&next_result, &JsValue::from_str("done"))?;
        if done.as_bool().unwrap_or(false) {
            break;
        }

        let value = Reflect::get(&next_result, &JsValue::from_str("value"))?;
        // value is [name: string, handle: FileSystemHandle]
        let pair = value
            .dyn_ref::<Array>()
            .ok_or_else(|| JsValue::from_str("OPFS: iterator value is not an Array"))?;

        if let Some(entry_name) = pair.get(0).as_string() {
            if entry_name.ends_with(".agentdb") {
                // Strip the ".agentdb" suffix to get the logical name
                let logical_name = entry_name
                    .strip_suffix(".agentdb")
                    .unwrap_or(&entry_name)
                    .to_string();
                names.push(logical_name);
            }
        }
    }

    Ok(names)
}

// ── Legacy stub — kept for compatibility ────────────────────────────────────

/// Stub VFS adapter — retained for documentation purposes.
///
/// The full-VFS approach (registering a custom `sqlite3_vfs` that delegates to
/// `FileSystemSyncAccessHandle` in a dedicated Worker) is architecturally
/// superior for large databases but requires a Worker thread. The load-save
/// pattern implemented above handles the common case without that complexity.
#[cfg(feature = "wasm")]
pub struct OpfsVfs {
    /// Name that would be passed to `sqlite3_vfs_register`.
    pub vfs_name: String,
}

#[cfg(feature = "wasm")]
impl OpfsVfs {
    /// Create a new `OpfsVfs` stub.
    pub fn new(vfs_name: &str) -> Self {
        OpfsVfs {
            vfs_name: vfs_name.to_string(),
        }
    }
}
