//! Keyboard shortcuts, resolved to intent before anything is drawn. This maps
//! a key press to an [`Action`] and nothing else, and it runs after the panels
//! are drawn, so a key a text field took never turns a page underneath it.

use eframe::egui::{Key, Modifiers};

/// What a key press asks the reader to do.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    /// Step forward by the layout's stride.
    NextPage,
    /// Step back by the layout's stride.
    PreviousPage,
    /// Jump to the first page.
    FirstPage,
    /// Jump to the last page.
    LastPage,
    /// Raise the resolution.
    ZoomIn,
    /// Lower the resolution.
    ZoomOut,
    /// Return to the default resolution.
    ZoomReset,
    /// Move to the next search hit.
    NextHit,
    /// Move to the previous search hit.
    PreviousHit,
    /// Put the caret in the search box.
    FocusSearch,
    /// Show or hide the search panel.
    ToggleSearch,
    /// Show or hide the signature panel.
    ToggleSignatures,
    /// Leave whatever has focus, closing a panel or the search box.
    Escape,
    /// Ask the host to open a document.
    Open,
    /// Ask the host to write a PDF of the pages on screen.
    ExportPdf,
    /// Ask the host to print the pages on screen.
    Print,
}

/// How much one zoom step moves the resolution, in dots per inch.
pub const ZOOM_STEP: f32 = 12.0;

/// Whether the shortcut depends on a document being open.
fn needs_document(action: Action) -> bool {
    !matches!(action, Action::Open | Action::Escape | Action::ToggleSearch)
}

/// Resolve one key press, or `None` if it means nothing to the reader. The
/// flag gates shortcuts that act on a page, so a key does nothing rather than
/// failing when there is no document to act on.
pub fn interpret(key: Key, modifiers: Modifiers, has_document: bool) -> Option<Action> {
    let action = action_for(key, modifiers)?;
    if !needs_document(action) || has_document {
        Some(action)
    } else {
        None
    }
}

/// The mapping itself, without the document check.
fn action_for(key: Key, modifiers: Modifiers) -> Option<Action> {
    let ctrl = modifiers.ctrl || modifiers.command;
    if ctrl {
        return match key {
            Key::O => Some(Action::Open),
            Key::F => Some(Action::ToggleSearch),
            Key::S => Some(Action::ExportPdf),
            Key::P => Some(Action::Print),
            // The browser's find shortcut. The key carries a slash, not the
            // question mark some layouts print on it: the binding follows the
            // hardware, because the layout is not ours to interpret.
            Key::Slash => Some(Action::FocusSearch),
            Key::Home => Some(Action::FirstPage),
            Key::End => Some(Action::LastPage),
            _ => None,
        };
    }
    match key {
        Key::ArrowRight | Key::PageDown => Some(Action::NextPage),
        Key::ArrowLeft | Key::PageUp => Some(Action::PreviousPage),
        Key::Home => Some(Action::FirstPage),
        Key::End => Some(Action::LastPage),
        Key::ArrowDown if modifiers.shift => Some(Action::NextHit),
        Key::ArrowUp if modifiers.shift => Some(Action::PreviousHit),
        Key::Equals | Key::Plus => Some(Action::ZoomIn),
        Key::Minus => Some(Action::ZoomOut),
        Key::Num0 => Some(Action::ZoomReset),
        Key::F3 => Some(if modifiers.shift {
            Action::PreviousHit
        } else {
            Action::NextHit
        }),
        Key::Slash if modifiers.ctrl => Some(Action::FocusSearch),
        Key::Escape => Some(Action::Escape),
        _ => None,
    }
}

/// What Escape should do, given what is open. The search box takes focus
/// before the panel opens, so Escape leaves the box first and closes the panel
/// on the next press. `None` means there is nothing to escape from.
pub fn escape_target(search_open: bool, search_focused: bool) -> Option<Action> {
    if search_focused {
        Some(Action::FocusSearch)
    } else if search_open {
        Some(Action::ToggleSearch)
    } else {
        None
    }
}

/// The search box's persistent id, shared by the widget and the focus request.
pub fn search_box_id() -> eframe::egui::Id {
    eframe::egui::Id::new("reader-search-box")
}
