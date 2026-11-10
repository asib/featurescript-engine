//! Files uploaded to an Onshape document (fixtures/blobs), as the engine finds them: a tab's element id and the
//! file on disk. A reference to one (Onshape's own Import feature's `blobData`) is what `@opImportForeign` reads.

use std::cell::RefCell;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

thread_local! {
    static UPLOADS: RefCell<HashMap<String, PathBuf>> = RefCell::new(HashMap::new());
}

/// Makes the file stand for the uploaded tab with this element id.
pub fn register(element_id: &str, file: &Path) {
    UPLOADS.with(|u| u.borrow_mut().insert(element_id.to_string(), file.to_path_buf()));
}

/// The file uploaded as the tab with this element id, if any.
pub fn file(element_id: &str) -> Option<PathBuf> {
    UPLOADS.with(|u| u.borrow().get(element_id).cloned())
}
