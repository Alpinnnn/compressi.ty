use eframe::egui::{
    self, Color32, ColorImage, CornerRadius, Id, Rect, RichText, ScrollArea, Sense, Stroke,
    StrokeKind, TextureHandle, TextureOptions, Ui, pos2, vec2,
};

use crate::{
    icons,
    modules::compress_documents::models::{
        DocumentCompressionState, DocumentKind, DocumentQueueItem,
    },
    theme::AppTheme,
    ui::components::{hint, panel},
};

use super::{CompressDocumentsPage, compact, flush, format_bytes, truncate_filename};

enum QueueAction {
    Compress(u64),
    Remove(u64),
    Select(u64),
}

struct QueueCategory {
    title: &'static str,
    tint: Color32,
    matches_state: fn(&DocumentCompressionState) -> bool,
}

impl CompressDocumentsPage {
    pub(super) fn render_queue(&mut self, ui: &mut Ui, theme: &AppTheme, height: f32) {
        panel::card(theme)
            .inner_margin(egui::Margin::same(16))
            .show(ui, |ui| {
                compact(ui);
                ui.set_min_height((height - 32.0).max(0.0));
                hint::title(
                    ui,
                    theme,
                    "Queue",
                    15.0,
                    Some("Run one document from its row or compress every ready item at once."),
                );
                ui.add_space(8.0);

                if self.queue.is_empty() {
                    ui.add_space(18.0);
                    ui.vertical_centered(|ui| {
                        ui.label(icons::rich(icons::DOCUMENT, 32.0, theme.colors.fg_dim));
                        ui.label(
                            RichText::new("No documents queued")
                                .size(13.0)
                                .color(theme.colors.fg_dim),
                        );
                    });
                    return;
                }

                let mut action = None;
                let categories = queue_categories(theme);
                ScrollArea::vertical()
                    .id_salt("document_queue_scroll")
                    .auto_shrink([false, false])
                    .max_height((height - 78.0).max(120.0))
                    .show(ui, |ui| {
                        flush(ui);
                        ui.set_width(ui.available_width());

                        for category in categories {
                            let indices = self
                                .queue
                                .iter()
                                .enumerate()
                                .filter(|(_, item)| (category.matches_state)(&item.state))
                                .map(|(index, _)| index)
                                .collect::<Vec<_>>();
                            if indices.is_empty() {
                                continue;
                            }

                            queue_section_header(
                                ui,
                                theme,
                                category.title,
                                indices.len(),
                                category.tint,
                            );
                            for index in indices {
                                let icon_key = document_icon_key(&self.queue[index]);
                                let icon_texture = self.document_icon_texture(ui, icon_key);
                                let item = &self.queue[index];
                                if let Some(row_action) = render_queue_row(
                                    ui,
                                    theme,
                                    item,
                                    icon_texture.as_ref(),
                                    self.selected_id == Some(item.asset.id),
                                    self.active_batch.is_some(),
                                ) {
                                    action = Some(row_action);
                                }
                                ui.add_space(8.0);
                            }
                        }
                    });

                match action {
                    Some(QueueAction::Compress(id)) => self.start_single_compression(id),
                    Some(QueueAction::Remove(id)) => self.remove_document(id),
                    Some(QueueAction::Select(id)) => {
                        self.selected_id = Some(id);
                    }
                    None => {}
                }
            });
    }

    fn document_icon_texture(&mut self, ui: &Ui, key: &'static str) -> Option<TextureHandle> {
        if !self.document_icon_textures.contains_key(key) {
            let bytes = document_icon_bytes(key)?;
            let texture = load_document_icon_texture(ui, key, bytes)?;
            self.document_icon_textures.insert(key, texture);
        }
        self.document_icon_textures.get(key).cloned()
    }
}

fn queue_categories(theme: &AppTheme) -> [QueueCategory; 3] {
    [
        QueueCategory {
            title: "Queue",
            tint: theme.colors.fg_muted,
            matches_state: is_queued,
        },
        QueueCategory {
            title: "Progress",
            tint: theme.colors.accent,
            matches_state: is_in_progress,
        },
        QueueCategory {
            title: "Done",
            tint: theme.colors.positive,
            matches_state: is_done,
        },
    ]
}

fn queue_section_header(ui: &mut Ui, theme: &AppTheme, title: &str, count: usize, tint: Color32) {
    ui.add_space(4.0);
    ui.label(
        RichText::new(format!("{title} - {count}"))
            .size(12.0)
            .strong()
            .color(tint),
    );
    let width = ui.available_width();
    let (line_rect, _) = ui.allocate_exact_size(vec2(width, 1.0), Sense::hover());
    ui.painter().rect_filled(
        line_rect,
        CornerRadius::ZERO,
        theme.mix(theme.colors.border, tint, 0.30),
    );
    ui.add_space(4.0);
}

fn is_queued(state: &DocumentCompressionState) -> bool {
    matches!(state, DocumentCompressionState::Ready)
}

fn is_in_progress(state: &DocumentCompressionState) -> bool {
    matches!(state, DocumentCompressionState::Compressing(_))
}

fn is_done(state: &DocumentCompressionState) -> bool {
    matches!(
        state,
        DocumentCompressionState::Completed(_)
            | DocumentCompressionState::Failed(_)
            | DocumentCompressionState::Cancelled
    )
}

fn render_queue_row(
    ui: &mut Ui,
    theme: &AppTheme,
    item: &DocumentQueueItem,
    icon_texture: Option<&TextureHandle>,
    selected: bool,
    batch_active: bool,
) -> Option<QueueAction> {
    let mut action = None;
    let row_id = Id::new("document_queue_row").with(item.asset.id);
    let row_width = ui.available_width();
    let row_height = 72.0;
    let (row_rect, _) = ui.allocate_exact_size(vec2(row_width, row_height), Sense::hover());
    let row_response = ui.interact(row_rect, row_id.with("select"), Sense::click());
    let pointer_inside_row = ui.ctx().input(|input| {
        input
            .pointer
            .hover_pos()
            .or_else(|| input.pointer.interact_pos())
            .map(|position| row_rect.contains(position))
            .unwrap_or(false)
    });

    let button_size = vec2(26.0, 24.0);
    let button_y = row_rect.center().y - button_size.y * 0.5;
    let delete_rect = Rect::from_min_size(
        pos2(row_rect.right() - button_size.x - 8.0, button_y),
        button_size,
    );
    let can_delete = !batch_active;
    let show_delete_button = can_delete && pointer_inside_row;
    let can_compress = can_start(&item.state) && !batch_active;
    let show_compress_button = can_compress && pointer_inside_row;
    let compress_rect = if show_compress_button {
        Some(Rect::from_min_size(
            pos2(
                delete_rect.left() - button_size.x - if show_delete_button { 6.0 } else { 0.0 },
                button_y,
            ),
            button_size,
        ))
    } else {
        None
    };

    let delete_response = if show_delete_button {
        Some(ui.interact(delete_rect, row_id.with("delete"), Sense::click()))
    } else {
        None
    };
    let compress_response = compress_rect
        .filter(|_| show_compress_button)
        .map(|rect| ui.interact(rect, row_id.with("compress"), Sense::click()));

    let fill = if selected {
        theme.mix(theme.colors.surface_hover, theme.colors.accent, 0.12)
    } else {
        theme.colors.bg_raised
    };
    let stroke = if selected {
        theme.colors.border_focus
    } else {
        theme.colors.border
    };
    let title_color = if selected {
        theme.colors.accent
    } else {
        theme.colors.fg
    };

    ui.painter().rect_filled(row_rect, CornerRadius::ZERO, fill);
    ui.painter().rect_stroke(
        row_rect,
        CornerRadius::ZERO,
        Stroke::new(
            1.0,
            if pointer_inside_row || selected {
                theme.colors.border_focus
            } else {
                stroke
            },
        ),
        StrokeKind::Middle,
    );

    let icon_rect = Rect::from_min_size(
        pos2(row_rect.left() + 10.0, row_rect.top() + 12.0),
        vec2(40.0, 40.0),
    );
    ui.painter()
        .rect_filled(icon_rect, CornerRadius::ZERO, theme.colors.bg_base);
    ui.painter().rect_stroke(
        icon_rect,
        CornerRadius::ZERO,
        Stroke::new(1.0, theme.colors.border),
        StrokeKind::Middle,
    );
    if let Some(texture) = icon_texture {
        let image_rect = icon_rect.shrink(4.0);
        ui.painter().image(
            texture.id(),
            image_rect,
            Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)),
            Color32::WHITE,
        );
    } else {
        ui.painter().text(
            icon_rect.center(),
            egui::Align2::CENTER_CENTER,
            icons::DOCUMENT,
            icons::font_id(16.0),
            theme.colors.fg_muted,
        );
    }

    let text_left = icon_rect.right() + 8.0;
    let action_left = compress_rect
        .map(|rect| rect.left())
        .unwrap_or(if show_delete_button {
            delete_rect.left()
        } else {
            row_rect.right() - 10.0
        });
    let text_width = (action_left - text_left - 6.0).max(0.0);
    let mut text_top = row_rect.top() + 11.0;

    let name = ui.painter().layout(
        truncate_filename(&item.asset.file_name, 30),
        egui::FontId::proportional(12.5),
        title_color,
        text_width,
    );
    ui.painter()
        .galley(pos2(text_left, text_top), name, title_color);
    text_top += 18.0;

    let info = ui.painter().layout_no_wrap(
        format_bytes(item.asset.original_size),
        egui::FontId::proportional(10.5),
        theme.colors.fg_dim,
    );
    ui.painter()
        .galley(pos2(text_left, text_top), info, theme.colors.fg_dim);
    text_top += 14.0;
    paint_document_state(ui, theme, &item.state, text_left, text_top, text_width);

    if let (Some(rect), Some(response)) = (compress_rect, &compress_response) {
        paint_icon_button(
            ui,
            theme,
            rect,
            response,
            icons::PLAY,
            theme.colors.accent,
            Color32::BLACK,
        );
        if response.clicked() {
            action = Some(QueueAction::Compress(item.asset.id));
        }
    }

    if let Some(response) = &delete_response {
        paint_icon_button(
            ui,
            theme,
            delete_rect,
            response,
            icons::TRASH,
            theme.mix(theme.colors.bg_raised, theme.colors.negative, 0.16),
            theme.colors.negative,
        );
        if response.clicked() {
            action = Some(QueueAction::Remove(item.asset.id));
        }
    }

    if row_response.clicked() && action.is_none() {
        action = Some(QueueAction::Select(item.asset.id));
    }

    action
}

fn paint_document_state(
    ui: &mut Ui,
    theme: &AppTheme,
    state: &DocumentCompressionState,
    left: f32,
    top: f32,
    width: f32,
) {
    match state {
        DocumentCompressionState::Ready => {
            paint_text(ui, "Ready", left, top, width, theme.colors.fg_muted);
        }
        DocumentCompressionState::Compressing(progress) => {
            let bar_width = width.max(20.0);
            let bar_rect = Rect::from_min_size(pos2(left, top + 14.0), vec2(bar_width, 4.0));
            paint_text(ui, &progress.stage, left, top, width, theme.colors.accent);
            ui.painter()
                .rect_filled(bar_rect, CornerRadius::same(2), theme.colors.bg_base);
            if progress.progress > 0.0 {
                let fill_rect = Rect::from_min_size(
                    bar_rect.min,
                    vec2(bar_rect.width() * progress.progress.clamp(0.0, 1.0), 4.0),
                );
                ui.painter()
                    .rect_filled(fill_rect, CornerRadius::same(2), theme.colors.accent);
            }
        }
        DocumentCompressionState::Completed(result) => {
            let color = if result.reduction_percent >= 0.0 {
                theme.colors.positive
            } else {
                theme.colors.caution
            };
            paint_text(
                ui,
                &format!(
                    "{} -> {} | {:.1}% saved",
                    format_bytes(result.original_size),
                    format_bytes(result.compressed_size),
                    result.reduction_percent
                ),
                left,
                top,
                width,
                color,
            );
        }
        DocumentCompressionState::Failed(error) => {
            paint_text(
                ui,
                &truncate_filename(error, 72),
                left,
                top,
                width,
                theme.colors.negative,
            );
        }
        DocumentCompressionState::Cancelled => {
            paint_text(ui, "Cancelled", left, top, width, theme.colors.caution);
        }
    }
}

fn paint_text(ui: &mut Ui, text: &str, left: f32, top: f32, width: f32, color: Color32) {
    let galley = ui.painter().layout(
        text.to_owned(),
        egui::FontId::proportional(10.5),
        color,
        width,
    );
    ui.painter().galley(pos2(left, top), galley, color);
}

fn paint_icon_button(
    ui: &mut Ui,
    theme: &AppTheme,
    rect: Rect,
    response: &egui::Response,
    icon: char,
    fill: Color32,
    icon_color: Color32,
) {
    let animation = ui.ctx().animate_bool(response.id, response.hovered());
    ui.painter().rect_filled(
        rect,
        CornerRadius::ZERO,
        theme.mix(fill, Color32::WHITE, animation * 0.10),
    );
    ui.painter().rect_stroke(
        rect,
        CornerRadius::ZERO,
        Stroke::new(1.0, theme.colors.border),
        StrokeKind::Middle,
    );
    ui.painter().text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        icon,
        icons::font_id(13.0),
        icon_color,
    );
}

fn document_icon_key(item: &DocumentQueueItem) -> &'static str {
    let extension = item
        .asset
        .path
        .extension()
        .and_then(|value| value.to_str())
        .map(|value| value.to_ascii_lowercase());

    match extension.as_deref() {
        Some("pdf") => "pdf",
        Some("docx" | "docm" | "dotx" | "dotm") => "word",
        Some("xlsx" | "xlsm" | "xltx" | "xltm" | "xlam") => "excel",
        Some("pptx" | "pptm" | "potx" | "potm" | "ppsx" | "ppsm" | "ppam" | "sldx" | "sldm") => {
            "powerpoint"
        }
        Some(
            "odt" | "ott" | "oth" | "odm" | "ods" | "ots" | "odp" | "otp" | "odg" | "otg" | "odf"
            | "odc" | "odi" | "odb",
        ) => "odf",
        Some("epub") => "epub",
        Some(
            "xps" | "oxps" | "vsdx" | "vsdm" | "vsstx" | "vsstm" | "vssx" | "vssm" | "vstx"
            | "vstm",
        ) => "xps",
        _ => match item.asset.kind {
            DocumentKind::Pdf => "pdf",
            DocumentKind::MicrosoftOpenXml => "word",
            DocumentKind::OpenDocument => "odf",
            DocumentKind::OpenPackaging => "xps",
            DocumentKind::Epub => "epub",
        },
    }
}

fn document_icon_bytes(key: &str) -> Option<&'static [u8]> {
    match key {
        "pdf" => Some(include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/assets/icon/pdf_icon.png"
        ))),
        "word" => Some(include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/assets/icon/word_icon.png"
        ))),
        "excel" => Some(include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/assets/icon/excel_icon.png"
        ))),
        "powerpoint" => Some(include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/assets/icon/powerpoint_icon.png"
        ))),
        "odf" => Some(include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/assets/icon/odf_icon.png"
        ))),
        "epub" => Some(include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/assets/icon/epub_icon.png"
        ))),
        "xps" => Some(include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/assets/icon/xps_icon.png"
        ))),
        _ => None,
    }
}

fn load_document_icon_texture(ui: &Ui, key: &'static str, bytes: &[u8]) -> Option<TextureHandle> {
    let image = image::load_from_memory(bytes).ok()?.to_rgba8();
    let size = [image.width() as usize, image.height() as usize];
    let color_image = ColorImage::from_rgba_unmultiplied(size, image.as_raw());
    Some(ui.ctx().load_texture(
        format!("document_type_icon_{key}"),
        color_image,
        TextureOptions::LINEAR,
    ))
}

fn can_start(state: &DocumentCompressionState) -> bool {
    matches!(
        state,
        DocumentCompressionState::Ready
            | DocumentCompressionState::Failed(_)
            | DocumentCompressionState::Cancelled
    )
}
