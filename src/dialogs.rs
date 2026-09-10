//! App-modal dialogs (spec §6.12, §7.2, §8.1).
//!
//! Exactly one modal exists at a time (an enum, not a stack). While a modal is
//! shown the rest of the app is not interactive, so guards can never stack.

use std::path::PathBuf;

pub enum Modal {
    /// Close a single dirty tab (Save / Discard / Cancel).
    CloseDirty { idx: usize },
    /// Window close / quit with N dirty buffers (Save All / Discard All / Cancel).
    QuitDirty { idxs: Vec<usize> },
    /// File changed on disk while our buffer has unsaved edits (Reload / Keep / Cancel).
    ExternalChanged { idx: usize },
    /// Insert link with an interactive editor (label + url).
    InsertLink { idx: usize },
    /// Insert image reference.
    InsertImage { idx: usize, path: PathBuf },
    /// Table grid picker (rows × columns).
    TablePicker { idx: usize },
    /// Crash-recovery choices from the last session (spec F68/F91).
    Recovery { items: Vec<(String, Option<PathBuf>, String)> },
    /// Jump to a line number.
    GoTo { idx: usize },
    About,
}

impl Modal {
    /// Is this modal one of the destructive-guard family?
    pub fn is_guard(&self) -> bool {
        matches!(self, Modal::CloseDirty { .. } | Modal::QuitDirty { .. } | Modal::ExternalChanged { .. })
    }
}

/// Rows chosen in the table picker before confirming.
pub struct TableDraft {
    pub rows: usize,
    pub cols: usize,
}
