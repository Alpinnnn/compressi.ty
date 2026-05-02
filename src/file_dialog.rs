use std::{
    path::PathBuf,
    sync::mpsc::{self, Receiver, TryRecvError},
    thread,
};

use eframe::egui;

/// File extension filter used by native file picker requests.
#[derive(Clone, Copy)]
pub(crate) struct FileDialogFilter {
    name: &'static str,
    extensions: &'static [&'static str],
}

impl FileDialogFilter {
    /// Creates a file extension filter with a display name and accepted extensions.
    pub(crate) const fn new(name: &'static str, extensions: &'static [&'static str]) -> Self {
        Self { name, extensions }
    }
}

/// Receives the eventual result of a native dialog without blocking egui's event loop.
pub(crate) type DialogReceiver<T> = Receiver<Option<T>>;

/// Opens a native multi-file picker on a worker thread.
pub(crate) fn pick_files(
    ctx: &egui::Context,
    title: &'static str,
    filters: Vec<FileDialogFilter>,
) -> Option<DialogReceiver<Vec<PathBuf>>> {
    spawn_dialog(ctx, "compressity-file-picker", move || {
        let dialog = filters
            .iter()
            .fold(rfd::FileDialog::new().set_title(title), |dialog, filter| {
                dialog.add_filter(filter.name, filter.extensions)
            });
        dialog.pick_files()
    })
}

/// Opens a native folder picker on a worker thread.
pub(crate) fn pick_folder(
    ctx: &egui::Context,
    title: &'static str,
) -> Option<DialogReceiver<PathBuf>> {
    spawn_dialog(ctx, "compressity-folder-picker", move || {
        rfd::FileDialog::new().set_title(title).pick_folder()
    })
}

/// Polls a pending native dialog result, clearing the receiver when it completes or disconnects.
pub(crate) fn poll_dialog<T>(receiver: &mut Option<DialogReceiver<T>>) -> Option<Option<T>> {
    let result = match receiver.as_ref() {
        Some(receiver) => receiver.try_recv(),
        None => return None,
    };

    match result {
        Ok(result) => {
            *receiver = None;
            Some(result)
        }
        Err(TryRecvError::Empty) => None,
        Err(TryRecvError::Disconnected) => {
            *receiver = None;
            Some(None)
        }
    }
}

fn spawn_dialog<T, F>(
    ctx: &egui::Context,
    thread_name: &'static str,
    open_dialog: F,
) -> Option<DialogReceiver<T>>
where
    T: Send + 'static,
    F: FnOnce() -> Option<T> + Send + 'static,
{
    let (sender, receiver) = mpsc::channel();
    let repaint_ctx = ctx.clone();
    let spawn_result = thread::Builder::new()
        .name(thread_name.to_owned())
        .spawn(move || {
            let result = open_dialog();
            let _ = sender.send(result);
            repaint_ctx.request_repaint();
        });

    if let Err(error) = spawn_result {
        eprintln!("failed to open native file dialog: {error}");
        return None;
    }

    Some(receiver)
}
