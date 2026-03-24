use std::{
    path::{Path, PathBuf},
    sync::mpsc,
    time::Duration,
};

use eframe::egui::{self, ColorImage, TextureOptions};

use crate::modules::compress_photos::models::{LoadedPhoto, PhotoPreview};

use super::super::{
    CompressPhotosPage, PhotoListItem, PreviewLoadEvent, PreviewLoadKind, models::CompressionState,
};

impl CompressPhotosPage {
    pub(super) fn apply_pending_loaded_photos(&mut self, ctx: &egui::Context) {
        for photo in self.pending_loaded_photos.drain(..) {
            self.files.push(make_item(ctx, photo));
        }
    }

    pub(super) fn poll_preview_loader(&mut self, ctx: &egui::Context) {
        let Some(rx) = &self.preview_loader_rx else {
            return;
        };

        loop {
            match rx.try_recv() {
                Ok(event) => match event.kind {
                    PreviewLoadKind::Progress => {
                        self.preview_load_progress = event.progress;
                    }
                    PreviewLoadKind::Input => {
                        self.preview_load_progress = event.progress;
                        if let Some(preview) = event.preview {
                            let texture = load_preview_texture(
                                ctx,
                                format!("preview-{}-input", event.id),
                                preview,
                            );
                            self.preview_input_texture = Some((event.id, texture));
                        }
                    }
                    PreviewLoadKind::Output => {
                        self.preview_load_progress = 1.0;
                        if let Some(preview) = event.preview {
                            let texture = load_preview_texture(
                                ctx,
                                format!("preview-{}-output", event.id),
                                preview,
                            );
                            self.preview_output_texture = Some((event.id, texture));
                        }
                    }
                },
                Err(mpsc::TryRecvError::Empty) => break,
                Err(mpsc::TryRecvError::Disconnected) => {
                    let had_missing_output = self.preview_output_texture.is_none()
                        && self.preview_input_texture.is_some();
                    if had_missing_output {
                        self.preview_output_failed = true;
                    }
                    self.preview_loading = false;
                    self.preview_load_progress = 1.0;
                    self.preview_loader_rx = None;
                    break;
                }
            }
        }

        if self.preview_loading {
            ctx.request_repaint_after(Duration::from_millis(30));
        }
    }

    pub(super) fn reset_preview_state(&mut self) {
        self.preview_input_texture = None;
        self.preview_output_texture = None;
        self.preview_loader_rx = None;
        self.preview_load_progress = 0.0;
        self.preview_loading = false;
        self.preview_output_failed = false;
    }

    pub(in crate::modules::compress_photos) fn spawn_preview_load(
        &mut self,
        ctx: &egui::Context,
        id: u64,
        input_path: PathBuf,
        output_path: Option<PathBuf>,
    ) {
        self.preview_loading = true;
        self.preview_load_progress = 0.0;
        self.preview_output_failed = false;
        let has_output = output_path.is_some();
        let (tx, rx) = mpsc::channel();
        self.preview_loader_rx = Some(rx);
        let repaint_ctx = ctx.clone();

        std::thread::spawn(move || {
            send_preview_event(&tx, &repaint_ctx, id, PreviewLoadKind::Progress, 0.08, None);

            let input_progress = if has_output { 0.5 } else { 0.95 };
            if let Some(preview) = read_preview_image(&input_path) {
                send_preview_event(
                    &tx,
                    &repaint_ctx,
                    id,
                    PreviewLoadKind::Input,
                    input_progress,
                    Some(preview),
                );
            }

            if let Some(output_path) = output_path {
                send_preview_event(&tx, &repaint_ctx, id, PreviewLoadKind::Progress, 0.60, None);

                if let Some(preview) = read_preview_image(&output_path) {
                    send_preview_event(
                        &tx,
                        &repaint_ctx,
                        id,
                        PreviewLoadKind::Output,
                        1.0,
                        Some(preview),
                    );
                }
            }
        });
    }
}

fn make_item(ctx: &egui::Context, photo: LoadedPhoto) -> PhotoListItem {
    let id = photo.asset.id;
    let preview_texture = photo
        .preview
        .map(|preview| load_preview_texture(ctx, format!("photo-preview-{id}"), preview));
    PhotoListItem {
        asset: photo.asset,
        preview_texture,
        state: CompressionState::Ready,
    }
}

fn load_preview_texture(
    ctx: &egui::Context,
    name: String,
    preview: PhotoPreview,
) -> egui::TextureHandle {
    let color_image = ColorImage::from_rgba_unmultiplied(preview.size, &preview.rgba);
    ctx.load_texture(name, color_image, TextureOptions::LINEAR)
}

fn read_preview_image(path: &Path) -> Option<PhotoPreview> {
    let rgba = image::open(path).ok()?.to_rgba8();
    Some(PhotoPreview {
        size: [rgba.width() as usize, rgba.height() as usize],
        rgba: rgba.into_raw(),
    })
}

fn send_preview_event(
    tx: &mpsc::Sender<PreviewLoadEvent>,
    repaint_ctx: &egui::Context,
    id: u64,
    kind: PreviewLoadKind,
    progress: f32,
    preview: Option<PhotoPreview>,
) {
    let _ = tx.send(PreviewLoadEvent {
        id,
        kind,
        preview,
        progress,
    });
    repaint_ctx.request_repaint();
}
