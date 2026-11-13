use crate::icons;
use crate::model::{self, ActiveCall, Body, CallEntry, Chat, Message, Status};
use crate::theme::{self, Palette};
use eframe::egui::{
    self, Align, Align2, Color32, FontId, Frame, Id, Layout, Margin, Pos2, Rect, Response,
    Rounding, ScrollArea, Sense, Stroke, TextEdit, Ui, Vec2,
};
use std::path::PathBuf;

#[derive(Clone, Copy, PartialEq, Eq)]
enum View {
    Chats,
    Calls,
    Link,
    Settings,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Filter {
    All,
    Unread,
    Groups,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Popup {
    None,
    Emoji,
    Attach,
}

struct Toast {
    text: String,
    until: f64,
}

struct PendingReply {
    chat_id: u64,
    due: f64,
    text: String,
}

struct SettingsState {
    read_receipts: bool,
    typing_indicators: bool,
    notifications: bool,
    sounds: bool,
}

pub struct FeatherApp {
    screenshot_path: Option<PathBuf>,
    frames: u64,
    requested_shot: bool,
    saved: bool,

    dark: bool,
    now: f64,
    chats: Vec<Chat>,
    selected: u64,
    view: View,
    filter: Filter,
    search: String,
    show_info: bool,
    show_picker: bool,
    picker_for_call: bool,
    picker_search: String,
    popup: Popup,
    show_menu: bool,
    menu_armed: bool,
    popup_armed: bool,
    menu_rect: Rect,
    popup_rect: Rect,
    toast: Option<Toast>,
    settings: SettingsState,
    calls: Vec<CallEntry>,
    call: Option<ActiveCall>,
    pending: Vec<PendingReply>,
    focus_composer: bool,
    link: crate::link::LinkState,
}

impl FeatherApp {
    pub fn new(cc: &eframe::CreationContext<'_>, screenshot_path: Option<String>) -> Self {
        theme::install_fonts(&cc.egui_ctx);
        let pal = Palette::light();
        pal.apply(&cc.egui_ctx);

        let taking_shot = screenshot_path.is_some();
        let mut app = Self {
            screenshot_path: screenshot_path.map(PathBuf::from),
            frames: 0,
            requested_shot: false,
            saved: false,
            dark: false,
            now: 0.0,
            chats: model::sample_chats(),
            selected: 1,
            view: View::Chats,
            filter: Filter::All,
            search: String::new(),
            show_info: false,
            show_picker: false,
            picker_for_call: false,
            picker_search: String::new(),
            popup: Popup::None,
            show_menu: false,
            menu_armed: false,
            popup_armed: false,
            menu_rect: Rect::NOTHING,
            popup_rect: Rect::NOTHING,
            toast: None,
            settings: SettingsState {
                read_receipts: true,
                typing_indicators: true,
                notifications: true,
                sounds: false,
            },
            calls: model::sample_calls(),
            call: None,
            pending: Vec::new(),
            focus_composer: false,
            link: crate::link::LinkState::new(),
        };

        let allow_resume = !taking_shot
            || std::env::var("FEATHER_RESUME").map(|v| v == "1").unwrap_or(false);
        if allow_resume && app.link.resume() {
            app.view = View::Chats;
        }

        if taking_shot && std::env::var("FEATHER_VIEW").map(|v| v == "link").unwrap_or(false)
        {
            app.view = View::Link;
            if let Ok(code) = std::env::var("FEATHER_HUB") {
                app.link.preview_join(&code, "You", "1234");
            }
        }

        app
    }

    fn pal(&self) -> Palette {
        if self.dark {
            Palette::dark()
        } else {
            Palette::light()
        }
    }

    fn set_dark(&mut self, ctx: &egui::Context, dark: bool) {
        self.dark = dark;
        self.pal().apply(ctx);
    }

    fn selected_index(&self) -> Option<usize> {
        self.chats.iter().position(|c| c.id == self.selected)
    }

    fn selected_chat(&self) -> Option<&Chat> {
        self.chats.iter().find(|c| c.id == self.selected)
    }

    fn toast(&mut self, text: impl Into<String>) {
        self.toast = Some(Toast {
            text: text.into(),
            until: self.now + 2.6,
        });
    }

    fn open_chat(&mut self, id: u64) {
        self.selected = id;
        self.view = View::Chats;
        self.show_info = false;
        self.show_picker = false;
        if let Some(chat) = self.chats.iter_mut().find(|c| c.id == id) {
            chat.unread = 0;
        }
    }

    fn handle_screenshot(&mut self, ctx: &egui::Context) {
        if self.screenshot_path.is_none() {
            return;
        }
        self.frames += 1;
        ctx.request_repaint();

        if self.frames == 120 && !self.requested_shot {
            self.requested_shot = true;
            ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot);
        }

        if self.requested_shot && !self.saved {
            let shot = ctx.input(|i| {
                i.events.iter().find_map(|e| {
                    if let egui::Event::Screenshot { image, .. } = e {
                        Some(image.clone())
                    } else {
                        None
                    }
                })
            });
            if let Some(img) = shot {
                let path = self.screenshot_path.clone().unwrap();
                let [w, h] = img.size;
                let mut rgba = Vec::with_capacity(w * h * 4);
                for px in &img.pixels {
                    rgba.extend_from_slice(&px.to_array());
                }
                if let Some(parent) = path.parent() {
                    let _ = std::fs::create_dir_all(parent);
                }
                let ok =
                    image::save_buffer(&path, &rgba, w as u32, h as u32, image::ColorType::Rgba8)
                        .is_ok();
                self.saved = true;
                if ok {
                    eprintln!("screenshot saved to {}", path.display());
                    std::process::exit(0);
                } else {
                    eprintln!("could not save screenshot to {}", path.display());
                    std::process::exit(1);
                }
            }
        }

        if self.frames > 400 && !self.saved {
            eprintln!("screenshot timed out");
            std::process::exit(1);
        }
    }

    fn process_time(&mut self, ctx: &egui::Context) {
        self.now = ctx.input(|i| i.time);
        let toast_expired = self
            .toast
            .as_ref()
            .map(|t| self.now > t.until)
            .unwrap_or(false);
        if toast_expired {
            self.toast = None;
        } else if self.toast.is_some() {
            ctx.request_repaint_after(std::time::Duration::from_millis(120));
        }

        let due_index = self
            .pending
            .iter()
            .position(|p| self.now >= p.due);
        if let Some(i) = due_index {
            let reply = self.pending.remove(i);
            if let Some(chat) = self.chats.iter_mut().find(|c| c.id == reply.chat_id) {
                chat.typing = false;
                chat.updated += 1;
                chat.messages.push(Message {
                    body: Body::Text(reply.text),
                    out: false,
                    time: LocalNow::time(),
                    day: "Today".to_owned(),
                    status: Status::Read,
                    sender: if chat.is_group {
                        Some("Sam".to_owned())
                    } else {
                        None
                    },
                });
                for m in chat.messages.iter_mut().filter(|m| m.out) {
                    m.status = Status::Read;
                }
                if chat.id != self.selected {
                    chat.unread += 1;
                }
            }
        }

        if !self.pending.is_empty() {
            ctx.request_repaint_after(std::time::Duration::from_millis(150));
        }
        if self.call.is_some() {
            ctx.request_repaint_after(std::time::Duration::from_millis(400));
        }
        if self.chats.iter().any(|c| c.typing) {
            ctx.request_repaint_after(std::time::Duration::from_millis(90));
        }
    }

    fn send_text(&mut self) {
        let text = self.chats.iter().find(|c| c.id == self.selected)
            .map(|c| c.draft.trim().to_owned())
            .unwrap_or_default();
        if text.is_empty() {
            return;
        }
        if let Some(chat) = self.chats.iter_mut().find(|c| c.id == self.selected) {
            chat.draft.clear();
        }
        self.push_outgoing(Body::Text(text));
    }

    fn push_outgoing(&mut self, body: Body) {
        let Some(idx) = self.selected_index() else {
            return;
        };
        let time = LocalNow::time();
        {
            let chat = &mut self.chats[idx];
            chat.messages.push(Message {
                body,
                out: true,
                time,
                day: "Today".to_owned(),
                status: Status::Sent,
                sender: None,
            });
            chat.updated += 1;
            chat.unread = 0;
            chat.typing = true;
        }

        let lines = reply_lines(&self.chats[idx]);
        let pick = (self.chats[idx].messages.len() + self.now as usize) % lines.len();
        self.pending.push(PendingReply {
            chat_id: self.chats[idx].id,
            due: self.now + 2.2,
            text: lines[pick].to_owned(),
        });
        self.focus_composer = true;
    }

    fn send_photo(&mut self) {
        let tints = [
            Color32::from_rgb(111, 158, 178),
            Color32::from_rgb(196, 158, 112),
            Color32::from_rgb(132, 166, 132),
        ];
        let tint = tints[(self.chats.len() + self.now as usize) % tints.len()];
        self.push_outgoing(Body::Photo {
            caption: String::new(),
            tint,
        });
    }

    fn send_document(&mut self) {
        self.push_outgoing(Body::Document {
            name: "design-notes.pdf".to_owned(),
            size: "240 KB".to_owned(),
        });
    }

    fn send_voice(&mut self) {
        let secs = 4 + (self.now as u32 % 9);
        self.push_outgoing(Body::Voice { secs });
    }

    fn send_location(&mut self) {
        self.push_outgoing(Body::Location {
            place: "Elm Street Café".to_owned(),
        });
    }

    fn handle_escape(&mut self, ctx: &egui::Context) {
        if !ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            return;
        }
        if self.call.is_some() {
            self.call = None;
        } else if self.show_picker {
            self.show_picker = false;
        } else if self.popup != Popup::None {
            self.popup = Popup::None;
        } else if self.show_menu {
            self.show_menu = false;
        } else if self.show_info {
            self.show_info = false;
        }
    }

    fn rail(&mut self, ctx: &egui::Context) {
        let pal = self.pal();
        egui::SidePanel::left("rail")
            .exact_width(74.0)
            .resizable(false)
            .frame(Frame::none().fill(pal.rail))
            .show(ctx, |ui| {
                let rect = ui.max_rect();
                ui.painter().line_segment(
                    [
                        Pos2::new(rect.right() - 0.5, rect.top()),
                        Pos2::new(rect.right() - 0.5, rect.bottom()),
                    ],
                    Stroke::new(1.0_f32, pal.border),
                );

                ui.add_space(20.0);
                ui.vertical_centered(|ui| {
                    let (r, resp) = ui.allocate_exact_size(Vec2::splat(46.0), Sense::click());
                    paint_avatar(ui.painter(), r.shrink(3.0), "Y", Color32::from_rgb(52, 120, 104), false, &pal);
                    if resp
                        .on_hover_text("Your profile")
                        .on_hover_cursor(egui::CursorIcon::PointingHand)
                        .clicked()
                    {
                        self.view = View::Settings;
                    }
                });

                ui.add_space(26.0);
                ui.vertical_centered(|ui| {
                    if rail_button(ui, &pal, self.view == View::Chats, "Chats", |p, r, c| {
                        icons::chat_bubble(p, r, c, 1.7)
                    })
                    .clicked()
                    {
                        self.view = View::Chats;
                    }
                    ui.add_space(4.0);
                    if rail_button(ui, &pal, self.view == View::Calls, "Calls", |p, r, c| {
                        icons::phone(p, r, c, 1.5)
                    })
                    .clicked()
                    {
                        self.view = View::Calls;
                    }
                    ui.add_space(4.0);
                    let link_resp =
                        rail_button(ui, &pal, self.view == View::Link, "Link", |p, r, c| {
                            icons::globe(p, r, c, 1.4)
                        });
                    if self.link.connected() {
                        let dot = link_resp.rect.right_top() + Vec2::new(-9.0, 9.0);
                        ui.painter().circle_filled(dot, 4.0, pal.accent);
                    }
                    if link_resp.clicked() {
                        self.view = View::Link;
                    }
                    ui.add_space(4.0);
                    if rail_button(ui, &pal, self.view == View::Settings, "Settings", |p, r, c| {
                        icons::settings(p, r, c, 1.6)
                    })
                    .clicked()
                    {
                        self.view = View::Settings;
                    }
                });

                ui.with_layout(Layout::bottom_up(Align::Center), |ui| {
                    ui.add_space(16.0);
                    let (rect, resp) = ui.allocate_exact_size(Vec2::splat(44.0), Sense::click());
                    if resp.hovered() {
                        ui.painter().circle_filled(rect.center(), 21.0, pal.hover);
                    }
                    if self.dark {
                        icons::sun(ui.painter(), rect.shrink(13.0), pal.weak, 1.6);
                    } else {
                        icons::moon(ui.painter(), rect.shrink(13.0), pal.weak, pal.rail, 1.6);
                    }
                    if resp
                        .on_hover_text(if self.dark { "Light theme" } else { "Dark theme" })
                        .on_hover_cursor(egui::CursorIcon::PointingHand)
                        .clicked()
                    {
                        let target = !self.dark;
                        self.set_dark(ctx, target);
                    }
                    ui.add_space(12.0);
                    ui.label(
                        egui::RichText::new("LITE")
                            .font(theme::semibold(8.0))
                            .color(pal.weak),
                    );
                });
            });
    }

    fn chat_list_panel(&mut self, ctx: &egui::Context) {
        let pal = self.pal();
        egui::SidePanel::left("chat_list")
            .exact_width(354.0)
            .resizable(false)
            .frame(Frame::none().fill(pal.panel))
            .show(ctx, |ui| {
                let rect = ui.max_rect();
                ui.painter().line_segment(
                    [
                        Pos2::new(rect.right() - 0.5, rect.top()),
                        Pos2::new(rect.right() - 0.5, rect.bottom()),
                    ],
                    Stroke::new(1.0_f32, pal.border),
                );

                // Brand row
                ui.add_space(16.0);
                ui.horizontal(|ui| {
                    ui.add_space(18.0);
                    let (logo_rect, _) =
                        ui.allocate_exact_size(Vec2::splat(26.0), Sense::hover());
                    icons::leaf(ui.painter(), logo_rect, pal.accent, 1.5);
                    ui.add_space(8.0);
                    ui.vertical(|ui| {
                        ui.label(
                            egui::RichText::new("Feather")
                                .font(theme::semibold(21.0))
                                .color(pal.text),
                        );
                        ui.label(
                            egui::RichText::new("lightweight messaging")
                                .font(theme::font(9.5))
                                .color(pal.weak),
                        );
                    });
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        ui.add_space(14.0);
                        if icon_button(ui, &pal, 36.0, "Menu", |p, r, c| icons::dots(p, r, c, 1.7))
                            .clicked()
                        {
                            self.show_menu = !self.show_menu;
                            self.menu_armed = false;
                        }
                        ui.add_space(2.0);
                        if icon_button(ui, &pal, 36.0, "New chat", |p, r, c| {
                            icons::compose(p, r, c, 1.6)
                        })
                        .clicked()
                        {
                            self.picker_for_call = false;
                            self.show_picker = true;
                            self.picker_search.clear();
                        }
                    });
                });

                // Search
                ui.add_space(14.0);
                ui.horizontal(|ui| {
                    ui.add_space(16.0);
                    search_field(ui, &pal, &mut self.search, "Search chats", 322.0);
                });

                // Filters
                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    ui.add_space(18.0);
                    if chip(ui, &pal, "All", self.filter == Filter::All).clicked() {
                        self.filter = Filter::All;
                    }
                    if chip(ui, &pal, "Unread", self.filter == Filter::Unread).clicked() {
                        self.filter = Filter::Unread;
                    }
                    if chip(ui, &pal, "Groups", self.filter == Filter::Groups).clicked() {
                        self.filter = Filter::Groups;
                    }
                });
                ui.add_space(8.0);
                let connected = self.link.connected();
                Frame::none()
                    .fill(if connected { pal.accent_soft } else { pal.panel_alt })
                    .rounding(Rounding::same(9.0))
                    .inner_margin(Margin::symmetric(10.0, 7.0))
                    .show(ui, |ui| {
                        ui.set_width((ui.available_width() - 22.0).max(120.0));
                        ui.horizontal(|ui| {
                            let (globe, _) =
                                ui.allocate_exact_size(Vec2::splat(13.0), Sense::hover());
                            icons::globe(
                                ui.painter(),
                                globe,
                                if connected { pal.accent_deep } else { pal.weak },
                                1.2,
                            );
                            ui.add_space(2.0);
                            ui.label(
                                egui::RichText::new(if connected {
                                    "Feather Link is live"
                                } else {
                                    "Go real · link a space"
                                })
                                .font(theme::semibold(10.5))
                                .color(if connected { pal.accent_deep } else { pal.weak }),
                            );
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                if ui
                                    .add(
                                        egui::Button::new(
                                            egui::RichText::new("Open")
                                                .font(theme::semibold(10.0))
                                                .color(pal.accent_deep),
                                        )
                                        .frame(false),
                                    )
                                    .clicked()
                                {
                                    self.view = View::Link;
                                }
                            });
                        });
                    });
                ui.add_space(8.0);
                let line_y = ui.cursor().top();
                ui.painter().line_segment(
                    [
                        Pos2::new(ui.max_rect().left() + 16.0, line_y),
                        Pos2::new(ui.max_rect().right() - 16.0, line_y),
                    ],
                    Stroke::new(1.0_f32, pal.border),
                );
                ui.add_space(2.0);

                let footer_h = 34.0;
                let list_h = (ui.available_height() - footer_h).max(80.0);
                let mut clicked_chat: Option<u64> = None;

                ScrollArea::vertical()
                    .id_source("chat_scroll")
                    .max_height(list_h)
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.add_space(4.0);
                        let mut indices: Vec<usize> = self
                            .chats
                            .iter()
                            .enumerate()
                            .filter(|(_, c)| {
                                let f = match self.filter {
                                    Filter::All => true,
                                    Filter::Unread => c.unread > 0,
                                    Filter::Groups => c.is_group,
                                };
                                let q = self.search.trim().to_lowercase();
                                let s = q.is_empty()
                                    || c.name.to_lowercase().contains(&q)
                                    || c.messages.iter().any(|m| {
                                        m.preview().to_lowercase().contains(&q)
                                    });
                                f && s
                            })
                            .map(|(i, _)| i)
                            .collect();
                        indices.sort_by(|&a, &b| {
                            self.chats[b]
                                .pinned
                                .cmp(&self.chats[a].pinned)
                                .then(self.chats[b].updated.cmp(&self.chats[a].updated))
                        });                        if indices.is_empty() {
                            ui.add_space(40.0);
                            ui.vertical_centered(|ui| {
                                ui.label(
                                    egui::RichText::new("No chats found")
                                        .font(theme::font(14.0))
                                        .color(pal.weak),
                                );
                                ui.label(
                                    egui::RichText::new("Try a different search")
                                        .font(theme::font(11.0))
                                        .color(pal.weak),
                                );
                            });
                        }

                        for i in indices {
                            let chat = &self.chats[i];
                            ui.horizontal(|ui| {
                                ui.add_space(10.0);
                                let selected = chat.id == self.selected;
                                let (row, resp) = ui.allocate_exact_size(
                                    Vec2::new(ui.available_width() - 10.0, 74.0),
                                    Sense::click(),
                                );
                                let row = row.shrink2(Vec2::new(0.0, 1.0));
                                if selected {
                                    ui.painter().rect_filled(row, 10.0, pal.selected);
                                } else if resp.hovered() {
                                    ui.painter().rect_filled(row, 10.0, pal.hover);
                                }

                                let av = Rect::from_center_size(
                                    Pos2::new(row.left() + 34.0, row.center().y),
                                    Vec2::splat(48.0),
                                );
                                paint_avatar(
                                    ui.painter(),
                                    av,
                                    &chat.initials,
                                    chat.color,
                                    chat.online && !chat.is_group,
                                    &pal,
                                );

                                let text_x = row.left() + 68.0;
                                let clipped =
                                    ui.painter().with_clip_rect(row.shrink2(Vec2::new(8.0, 0.0)));
                                let name_text = ellipsize(
                                    ui,
                                    &chat.name,
                                    theme::semibold(14.5),
                                    (row.right() - 78.0 - text_x).max(30.0),
                                );
                                clipped.text(
                                    Pos2::new(text_x, row.top() + 15.0),
                                    Align2::LEFT_TOP,
                                    name_text,
                                    theme::semibold(14.5),
                                    pal.text,
                                );
                                clipped.text(
                                    Pos2::new(row.right() - 12.0, row.top() + 17.0),
                                    Align2::RIGHT_TOP,
                                    chat.time_label(),
                                    theme::font(10.5),
                                    if chat.unread > 0 { pal.accent } else { pal.weak },
                                );

                                let preview = if chat.typing {
                                    "typing…".to_owned()
                                } else {
                                    chat.preview()
                                };
                                let preview_max =
                                    (row.right() - 44.0 - text_x).max(30.0);
                                let preview_text = ellipsize(
                                    ui,
                                    &preview,
                                    theme::font(12.5),
                                    preview_max,
                                );
                                let preview_clip = Rect::from_min_max(
                                    Pos2::new(text_x, row.top() + 39.0),
                                    Pos2::new(row.right() - 44.0, row.bottom()),
                                );
                                ui.painter()
                                    .with_clip_rect(preview_clip)
                                    .text(
                                        Pos2::new(text_x, row.top() + 40.0),
                                        Align2::LEFT_TOP,
                                        preview_text,
                                        theme::font(12.5),
                                        if chat.typing { pal.accent } else { pal.weak },
                                    );

                                if chat.pinned {
                                    let pin_rect = Rect::from_min_size(
                                        Pos2::new(row.right() - 30.0, row.top() + 38.0),
                                        Vec2::splat(14.0),
                                    );
                                    icons::pin(
                                        ui.painter(),
                                        pin_rect,
                                        pal.weak,
                                        1.2,
                                    );
                                }

                                if chat.unread > 0 {
                                    let badge = Rect::from_center_size(
                                        Pos2::new(row.right() - 18.0, row.bottom() - 18.0),
                                        Vec2::new(
                                            if chat.unread > 9 { 24.0 } else { 20.0 },
                                            20.0,
                                        ),
                                    );
                                    ui.painter().rect_filled(
                                        badge,
                                        10.0,
                                        if chat.muted { pal.weak } else { pal.badge },
                                    );
                                    ui.painter().text(
                                        badge.center(),
                                        Align2::CENTER_CENTER,
                                        chat.unread.to_string(),
                                        theme::semibold(10.0),
                                        Color32::WHITE,
                                    );
                                }

                                if resp
                                    .on_hover_cursor(egui::CursorIcon::PointingHand)
                                    .clicked()
                                {
                                    clicked_chat = Some(chat.id);
                                }
                            });
                        }
                        ui.add_space(8.0);
                    });

                if let Some(id) = clicked_chat {
                    self.open_chat(id);
                }

                // Lightweight footer
                ui.with_layout(Layout::bottom_up(Align::Min), |ui| {
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        ui.add_space(18.0);
                        ui.label(
                            egui::RichText::new("●")
                                .font(theme::font(7.0))
                                .color(pal.accent),
                        );
                        ui.label(
                            egui::RichText::new("0 ads  ·  0 trackers  ·  stays small")
                                .font(theme::font(9.5))
                                .color(pal.weak),
                        );
                    });
                    ui.add_space(4.0);
                });
            });
    }

    fn chat_header(&mut self, ctx: &egui::Context) {
        let pal = self.pal();
        let Some(chat) = self.selected_chat() else {
            return;
        };
        let chat = chat.clone_shallow();
        egui::TopBottomPanel::top("chat_header")
            .exact_height(68.0)
            .frame(
                Frame::none()
                    .fill(pal.header)
                    .inner_margin(Margin {
                        left: 18.0,
                        right: 12.0,
                        top: 0.0,
                        bottom: 0.0,
                    }),
            )
            .show(ctx, |ui| {
                let rect = ui.max_rect();
                ui.painter().line_segment(
                    [
                        Pos2::new(rect.left(), rect.bottom() - 0.5),
                        Pos2::new(rect.right(), rect.bottom() - 0.5),
                    ],
                    Stroke::new(1.0_f32, pal.border),
                );
                ui.horizontal_centered(|ui| {
                    let (av, _) = ui.allocate_exact_size(Vec2::splat(42.0), Sense::hover());
                    paint_avatar(
                        ui.painter(),
                        av,
                        &chat.initials,
                        chat.color,
                        chat.online && !chat.is_group,
                        &pal,
                    );
                    ui.add_space(4.0);
                    ui.vertical(|ui| {
                        ui.horizontal(|ui| {
                            ui.label(
                                egui::RichText::new(&chat.name)
                                    .font(theme::semibold(15.5))
                                    .color(pal.text),
                            );
                            lite_badge(ui, &pal);
                        });
                        ui.label(
                            egui::RichText::new(chat.status_line())
                                .font(theme::font(11.0))
                                .color(if chat.typing { pal.accent } else { pal.weak }),
                        );
                    });

                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        ui.add_space(4.0);
                        if icon_button(ui, &pal, 38.0, "Contact info", |p, r, c| {
                            icons::info(p, r, c, 1.6)
                        })
                        .clicked()
                        {
                            self.show_info = !self.show_info;
                        }
                        if icon_button(ui, &pal, 38.0, "Voice call", |p, r, c| {
                            icons::phone(p, r, c, 1.5)
                        })
                        .clicked()
                        {
                            self.start_call(&chat, false);
                        }
                        if icon_button(ui, &pal, 38.0, "Video call", |p, r, c| {
                            icons::video(p, r, c, 1.5)
                        })
                        .clicked()
                        {
                            self.start_call(&chat, true);
                        }
                    });
                });
            });
    }

    fn start_call(&mut self, chat: &Chat, video: bool) {
        self.call = Some(ActiveCall {
            name: chat.name.clone(),
            initials: chat.initials.clone(),
            color: chat.color,
            video,
            started: self.now,
            muted: false,
            answered: None,
        });
        self.show_picker = false;
    }

    fn thread_panel(&mut self, ctx: &egui::Context) {
        let pal = self.pal();
        egui::CentralPanel::default()
            .frame(Frame::none().fill(pal.wallpaper))
            .show(ctx, |ui| {
                let rect = ui.max_rect();
                paint_wallpaper(ui.painter(), rect, &pal);
                let Some(chat) = self.selected_chat() else {
                    return;
                };
                let chat = chat.clone_shallow();

                ScrollArea::vertical()
                    .id_source(("thread", chat.id))
                    .auto_shrink([false, false])
                    .stick_to_bottom(true)
                    .show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.spacing_mut().item_spacing.y = 2.0;
                        ui.add_space(18.0);
                        let max_bubble = (ui.available_width() * 0.66).min(520.0);
                        let mut day = String::new();
                        let mut last_out: Option<bool> = None;
                        for msg in &chat.messages {
                            if msg.day != day {
                                day_pill(ui, &pal, &msg.day);
                                day = msg.day.clone();
                                last_out = None;
                            }
                            let tight = last_out == Some(msg.out);
                            message_bubble(ui, &pal, msg, max_bubble, tight);
                            if msg.day == day && !tight {
                                ui.add_space(5.0);
                            } else if tight {
                                ui.add_space(1.0);
                            }
                            last_out = Some(msg.out);
                        }
                        if chat.typing {
                            typing_bubble(ui, &pal, self.now);
                        }
                        ui.add_space(20.0);
                    });

                // Soften the top edge where the conversation slides under the header.
                for i in 0..6 {
                    let alpha = (70.0 * (1.0 - i as f32 / 6.0)) as u8;
                    let band = Rect::from_min_size(
                        Pos2::new(rect.left(), rect.top() + i as f32 * 5.0),
                        Vec2::new(rect.width(), 5.0),
                    );
                    ui.painter().rect_filled(
                        band,
                        0.0,
                        Color32::from_rgba_unmultiplied(
                            pal.wallpaper.r(),
                            pal.wallpaper.g(),
                            pal.wallpaper.b(),
                            alpha,
                        ),
                    );
                }
            });
    }

    fn composer(&mut self, ctx: &egui::Context) {
        let pal = self.pal();
        let Some(idx) = self.selected_index() else {
            return;
        };
        let mut draft = std::mem::take(&mut self.chats[idx].draft);
        let mut action: Option<u8> = None;

        egui::TopBottomPanel::bottom("composer")
            .exact_height(78.0)
            .frame(
                Frame::none()
                    .fill(pal.panel)
                    .inner_margin(Margin {
                        left: 14.0,
                        right: 16.0,
                        top: 12.0,
                        bottom: 12.0,
                    }),
            )
            .show(ctx, |ui| {
                let rect = ui.max_rect();
                ui.painter().line_segment(
                    [
                        Pos2::new(rect.left() - 14.0, rect.top() - 12.0),
                        Pos2::new(rect.right() + 16.0, rect.top() - 12.0),
                    ],
                    Stroke::new(1.0_f32, pal.border),
                );

                ui.horizontal_centered(|ui| {
                    if icon_button(ui, &pal, 38.0, "Emoji", |p, r, c| {
                        icons::smiley(p, r, c, 1.5)
                    })
                    .clicked()
                    {
                        self.popup = if self.popup == Popup::Emoji {
                            Popup::None
                        } else {
                            self.popup_armed = false;
                            Popup::Emoji
                        };
                    }
                    if icon_button(ui, &pal, 38.0, "Attach", |p, r, c| {
                        icons::paperclip(p, r, c, 1.5)
                    })
                    .clicked()
                    {
                        self.popup = if self.popup == Popup::Attach {
                            Popup::None
                        } else {
                            self.popup_armed = false;
                            Popup::Attach
                        };
                    }
                    ui.add_space(2.0);

                    let field_w = (ui.available_width() - 56.0).max(120.0);
                    Frame::none()
                        .fill(pal.search_bg)
                        .rounding(21.0)
                        .inner_margin(Margin::symmetric(16.0, 10.0))
                        .show(ui, |ui| {
                            ui.set_width(field_w - 32.0);
                            let resp = ui.add(
                                TextEdit::singleline(&mut draft)
                                    .frame(false)
                                    .hint_text("Type a message")
                                    .font(theme::font(14.0))
                                    .text_color(pal.text)
                                    .desired_width(f32::INFINITY),
                            );
                            if self.focus_composer {
                                resp.request_focus();
                                self.focus_composer = false;
                            }
                            if (resp.has_focus() || resp.lost_focus())
                                && ui.input(|i| i.key_pressed(egui::Key::Enter))
                            {
                                action = Some(if draft.trim().is_empty() { 2 } else { 1 });
                            }
                        });

                    ui.add_space(6.0);
                    let is_empty = draft.trim().is_empty();
                    let (send_rect, send_resp) =
                        ui.allocate_exact_size(Vec2::splat(44.0), Sense::click());
                    ui.painter().circle_filled(
                        send_rect.center(),
                        22.0,
                        if send_resp.hovered() {
                            pal.accent_deep
                        } else {
                            pal.accent
                        },
                    );
                    if is_empty {
                        icons::mic(
                            ui.painter(),
                            send_rect.shrink(12.0),
                            Color32::WHITE,
                            1.7,
                        );
                    } else {
                        icons::send(
                            ui.painter(),
                            send_rect.shrink(12.0),
                            Color32::WHITE,
                            1.5,
                        );
                    }
                    if send_resp
                        .on_hover_text(if is_empty {
                            "Record a voice note"
                        } else {
                            "Send"
                        })
                        .on_hover_cursor(egui::CursorIcon::PointingHand)
                        .clicked()
                    {
                        action = Some(if is_empty { 2 } else { 1 });
                    }
                });
            });

        self.chats[idx].draft = draft;
        match action {
            Some(1) => self.send_text(),
            Some(2) => self.send_voice(),
            _ => {}
        }
    }

    fn info_panel(&mut self, ctx: &egui::Context) {
        let pal = self.pal();
        let Some(idx) = self.selected_index() else {
            return;
        };
        let chat = self.chats[idx].clone_shallow();
        let mut close = false;
        let mut toggle_mute = false;
        let mut block = false;
        let mut call_now = false;

        egui::SidePanel::right("info_panel")
            .exact_width(310.0)
            .resizable(false)
            .frame(Frame::none().fill(pal.panel))
            .show(ctx, |ui| {
                let rect = ui.max_rect();
                ui.painter().line_segment(
                    [
                        Pos2::new(rect.left() + 0.5, rect.top()),
                        Pos2::new(rect.left() + 0.5, rect.bottom()),
                    ],
                    Stroke::new(1.0_f32, pal.border),
                );

                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    ui.add_space(14.0);
                    ui.label(
                        egui::RichText::new(if chat.is_group {
                            "Group info"
                        } else {
                            "Contact info"
                        })
                        .font(theme::semibold(16.0))
                        .color(pal.text),
                    );
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        ui.add_space(10.0);
                        if icon_button(ui, &pal, 34.0, "Close", |p, r, c| {
                            icons::close(p, r, c, 1.5)
                        })
                        .clicked()
                        {
                            close = true;
                        }
                    });
                });

                ui.add_space(14.0);
                ui.vertical_centered(|ui| {
                    let (av, _) = ui.allocate_exact_size(Vec2::splat(88.0), Sense::hover());
                    paint_avatar(
                        ui.painter(),
                        av,
                        &chat.initials,
                        chat.color,
                        chat.online && !chat.is_group,
                        &pal,
                    );
                    ui.add_space(10.0);
                    ui.label(
                        egui::RichText::new(&chat.name)
                            .font(theme::semibold(20.0))
                            .color(pal.text),
                    );
                    ui.label(
                        egui::RichText::new(if chat.is_group {
                            format!("group · {} members", chat.members)
                        } else {
                            chat.phone.clone()
                        })
                        .font(theme::font(12.0))
                        .color(pal.weak),
                    );
                });

                ui.add_space(22.0);
                ui.horizontal(|ui| {
                    ui.add_space(18.0);
                    ui.vertical(|ui| {
                        ui.label(
                            egui::RichText::new("ABOUT")
                                .font(theme::semibold(9.0))
                                .color(pal.weak),
                        );
                        ui.add_space(3.0);
                        ui.label(
                            egui::RichText::new(&chat.about)
                                .font(theme::font(13.0))
                                .color(pal.text),
                        );
                    });
                });

                ui.add_space(18.0);
                ui.horizontal(|ui| {
                    ui.add_space(18.0);
                    let card_w = 132.0;
                    if info_action_card(
                        ui,
                        &pal,
                        if chat.muted { "Unmute" } else { "Mute" },
                        if chat.muted {
                            "Notifs are off"
                        } else {
                            "Notifs are on"
                        },
                        |p, r, c| icons::bell(p, r, c, 1.4),
                        card_w,
                    )
                    .clicked()
                    {
                        toggle_mute = true;
                    }
                    ui.add_space(4.0);
                    if info_action_card(
                        ui,
                        &pal,
                        "Call",
                        "Voice call",
                        |p, r, c| icons::phone(p, r, c, 1.3),
                        card_w,
                    )
                    .clicked()
                    {
                        call_now = true;
                    }
                });

                ui.add_space(24.0);
                ui.horizontal(|ui| {
                    ui.add_space(18.0);
                    ui.label(
                        egui::RichText::new("MEDIA, LINKS AND DOCS")
                            .font(theme::semibold(9.0))
                            .color(pal.weak),
                    );
                });
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    ui.add_space(18.0);
                    for i in 0..3 {
                        let (r, _) = ui.allocate_exact_size(Vec2::splat(82.0), Sense::hover());
                        media_thumb(ui.painter(), r, &pal, i);
                        ui.add_space(4.0);
                    }
                });

                ui.add_space(22.0);
                ui.horizontal(|ui| {
                    ui.add_space(18.0);
                    if danger_button(
                        ui,
                        &pal,
                        if chat.is_group { "Exit group" } else { "Block contact" },
                    )
                    .clicked()
                    {
                        block = true;
                    }
                });
                ui.add_space(18.0);
            });

        if close {
            self.show_info = false;
        }
        if toggle_mute {
            if let Some(chat) = self.chats.iter_mut().find(|c| c.id == self.selected) {
                chat.muted = !chat.muted;
            }
        }
        if call_now {
            self.start_call(&chat, false);
        }
        if block {
            self.toast(if chat.is_group {
                "You left the group (demo)"
            } else {
                "Contact blocked in this demo"
            });
            self.show_info = false;
        }
    }

    fn calls_panel(&mut self, ctx: &egui::Context) {
        let pal = self.pal();
        egui::CentralPanel::default()
            .frame(Frame::none().fill(pal.panel))
            .show(ctx, |ui| {
                ui.add_space(26.0);
                ui.horizontal(|ui| {
                    ui.add_space(34.0);
                    ui.vertical(|ui| {
                        ui.label(
                            egui::RichText::new("Calls")
                                .font(theme::semibold(26.0))
                                .color(pal.text),
                        );
                        ui.label(
                            egui::RichText::new("Recent voice and video calls")
                                .font(theme::font(12.0))
                                .color(pal.weak),
                        );
                    });
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        ui.add_space(34.0);
                        if accent_button(ui, &pal, "Start a call").clicked() {
                            self.picker_for_call = true;
                            self.show_picker = true;
                            self.picker_search.clear();
                        }
                    });
                });
                ui.add_space(20.0);
                ui.horizontal(|ui| {
                    ui.add_space(34.0);
                    ui.painter().line_segment(
                        [
                            Pos2::new(ui.cursor().left(), ui.cursor().top()),
                            Pos2::new(ui.max_rect().right() - 34.0, ui.cursor().top()),
                        ],
                        Stroke::new(1.0_f32, pal.border),
                    );
                });
                ui.add_space(10.0);

                let mut start: Option<(String, String, Color32, bool)> = None;
                ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                    for entry in &self.calls {
                        ui.horizontal(|ui| {
                            ui.add_space(34.0);
                            let (row, resp) = ui.allocate_exact_size(
                                Vec2::new(ui.available_width() - 34.0, 74.0),
                                Sense::click(),
                            );
                            if resp.hovered() {
                                ui.painter().rect_filled(row, 10.0, pal.hover);
                            }
                            let av = Rect::from_center_size(
                                Pos2::new(row.left() + 30.0, row.center().y),
                                Vec2::splat(46.0),
                            );
                            paint_avatar(ui.painter(), av, &entry.initials, entry.color, false, &pal);

                            ui.painter().text(
                                Pos2::new(row.left() + 64.0, row.top() + 18.0),
                                Align2::LEFT_TOP,
                                &entry.name,
                                theme::semibold(14.5),
                                pal.text,
                            );
                            let tint = if entry.missed { pal.danger } else { pal.weak };
                            let arrow_r = Rect::from_min_size(
                                Pos2::new(row.left() + 64.0, row.top() + 43.0),
                                Vec2::splat(12.0),
                            );
                            if entry.incoming {
                                icons::arrow_out(ui.painter(), arrow_r, tint, 1.2);
                            } else {
                                icons::arrow_in(ui.painter(), arrow_r, tint, 1.2);
                            }
                            ui.painter().text(
                                Pos2::new(row.left() + 80.0, row.top() + 41.0),
                                Align2::LEFT_TOP,
                                &entry.when,
                                theme::font(11.0),
                                tint,
                            );

                            let center_y = row.center().y;
                            let video_box = Rect::from_center_size(
                                Pos2::new(row.right() - 34.0, center_y),
                                Vec2::splat(32.0),
                            );
                            let phone_box = Rect::from_center_size(
                                Pos2::new(row.right() - 78.0, center_y),
                                Vec2::splat(32.0),
                            );
                            icons::video(
                                ui.painter(),
                                video_box.shrink(8.0),
                                pal.accent,
                                1.3,
                            );
                            icons::phone(
                                ui.painter(),
                                phone_box.shrink(8.0),
                                pal.accent,
                                1.2,
                            );

                            let vid_rect = Rect::from_center_size(video_box.center(), Vec2::splat(36.0));
                            let vid_resp = ui.interact(
                                vid_rect,
                                Id::new(("call_video", &entry.name)),
                                Sense::click(),
                            );
                            let ph_rect = Rect::from_center_size(phone_box.center(), Vec2::splat(36.0));
                            let ph_resp = ui.interact(
                                ph_rect,
                                Id::new(("call_phone", &entry.name)),
                                Sense::click(),
                            );
                            if vid_resp.clicked() {
                                start = Some((
                                    entry.name.clone(),
                                    entry.initials.clone(),
                                    entry.color,
                                    true,
                                ));
                            } else if ph_resp.clicked() {
                                start = Some((
                                    entry.name.clone(),
                                    entry.initials.clone(),
                                    entry.color,
                                    false,
                                ));
                            }
                        });
                        ui.add_space(2.0);
                    }
                });

                if let Some((name, initials, color, video)) = start {
                    self.call = Some(ActiveCall {
                        name,
                        initials,
                        color,
                        video,
                        started: self.now,
                        muted: false,
                        answered: None,
                    });
                }
            });
    }

    fn settings_panel(&mut self, ctx: &egui::Context) {
        let pal = self.pal();
        let mut set_dark: Option<bool> = None;
        egui::CentralPanel::default()
            .frame(Frame::none().fill(pal.panel))
            .show(ctx, |ui| {
                ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                    let full_w = ui.available_width();
                    let content_w = (full_w - 88.0).clamp(420.0, 690.0);
                    let pad = ((full_w - content_w) * 0.5).max(44.0);
                    ui.add_space(28.0);
                    ui.horizontal(|ui| {
                        ui.add_space(pad);
                        ui.vertical(|ui| {
                            ui.label(
                                egui::RichText::new("Settings")
                                    .font(theme::semibold(26.0))
                                    .color(pal.text),
                            );
                            ui.label(
                                egui::RichText::new("Small by default. Only the essentials.")
                                    .font(theme::font(12.0))
                                    .color(pal.weak),
                            );
                        });
                    });
                    ui.add_space(22.0);

                    ui.horizontal(|ui| {
                        ui.add_space(pad);
                        ui.allocate_ui_with_layout(
                            Vec2::new(content_w, 88.0),
                            Layout::left_to_right(Align::Center),
                            |ui| {
                                Frame::none()
                                    .fill(pal.panel_alt)
                                    .rounding(14.0)
                                    .inner_margin(16.0)
                                    .show(ui, |ui| {
                                        ui.set_width(ui.available_width());
                                        ui.horizontal(|ui| {
                                            let (av, _) = ui
                                                .allocate_exact_size(Vec2::splat(58.0), Sense::hover());
                                            paint_avatar(
                                                ui.painter(),
                                                av,
                                                "Y",
                                                Color32::from_rgb(52, 120, 104),
                                                true,
                                                &pal,
                                            );
                                            ui.add_space(8.0);
                                            ui.vertical(|ui| {
                                                ui.label(
                                                    egui::RichText::new("You")
                                                        .font(theme::semibold(18.0))
                                                        .color(pal.text),
                                                );
                                                ui.label(
                                                    egui::RichText::new("Available · this device only")
                                                        .font(theme::font(11.5))
                                                        .color(pal.weak),
                                                );
                                            });
                                            ui.with_layout(
                                                Layout::right_to_left(Align::Center),
                                                |ui| {
                                                    ui.label(
                                                        egui::RichText::new("LITE")
                                                            .font(theme::semibold(9.0))
                                                            .color(pal.accent),
                                                    );
                                                },
                                            );
                                        });
                                    });
                            },
                        );
                    });

                    ui.add_space(24.0);
                    ui.horizontal(|ui| {
                        ui.add_space(pad);
                        ui.allocate_ui_with_layout(
                            Vec2::new(content_w, 0.0),
                            Layout::top_down(Align::Min),
                            |ui| {
                                ui.set_width(ui.available_width());
                                section_title(ui, &pal, "Appearance");
                                ui.add_space(8.0);
                                ui.horizontal(|ui| {
                                    if theme_tile(ui, &pal, false, !self.dark).clicked() {
                                        set_dark = Some(false);
                                    }
                                    ui.add_space(10.0);
                                    if theme_tile(ui, &pal, true, self.dark).clicked() {
                                        set_dark = Some(true);
                                    }
                                });

                                ui.add_space(22.0);
                                section_title(ui, &pal, "Privacy");
                                ui.add_space(8.0);
                                settings_card(ui, &pal, |ui| {
                                    toggle_row(
                                        ui,
                                        &pal,
                                        "Read receipts",
                                        "Share when you've read a message",
                                        &mut self.settings.read_receipts,
                                    );
                                    divider(ui, &pal);
                                    toggle_row(
                                        ui,
                                        &pal,
                                        "Typing indicators",
                                        "Let people see when you're writing",
                                        &mut self.settings.typing_indicators,
                                    );
                                });

                                ui.add_space(22.0);
                                section_title(ui, &pal, "Notifications");
                                ui.add_space(8.0);
                                settings_card(ui, &pal, |ui| {
                                    toggle_row(
                                        ui,
                                        &pal,
                                        "Message previews",
                                        "Show a notification when a chat is quiet",
                                        &mut self.settings.notifications,
                                    );
                                    divider(ui, &pal);
                                    toggle_row(
                                        ui,
                                        &pal,
                                        "Sounds",
                                        "A soft chime for new messages",
                                        &mut self.settings.sounds,
                                    );
                                });

                                ui.add_space(22.0);
                                section_title(ui, &pal, "Storage");
                                ui.add_space(8.0);
                                settings_card(ui, &pal, |ui| {
                                    ui.horizontal(|ui| {
                                        ui.label(
                                            egui::RichText::new("2.4 MB used")
                                                .font(theme::semibold(13.0))
                                                .color(pal.text),
                                        );
                                        ui.with_layout(
                                            Layout::right_to_left(Align::Center),
                                            |ui| {
                                                ui.label(
                                                    egui::RichText::new("3%")
                                                        .font(theme::font(11.0))
                                                        .color(pal.weak),
                                                );
                                            },
                                        );
                                    });
                                    ui.add_space(8.0);
                                    let (bar, _) = ui.allocate_exact_size(
                                        Vec2::new(ui.available_width(), 7.0),
                                        Sense::hover(),
                                    );
                                    ui.painter().rect_filled(bar, 4.0, pal.border);
                                    let fill = Rect::from_min_size(
                                        bar.min,
                                        Vec2::new((bar.width() * 0.03).max(10.0), bar.height()),
                                    );
                                    ui.painter().rect_filled(fill, 4.0, pal.accent);
                                    ui.add_space(9.0);
                                    ui.label(
                                        egui::RichText::new(
                                            "Messages live in memory here. Nothing is uploaded, and the cache clears itself.",
                                        )
                                        .font(theme::font(11.5))
                                        .color(pal.weak),
                                    );
                                });

                                ui.add_space(22.0);
                                section_title(ui, &pal, "About Feather");
                                ui.add_space(8.0);
                                settings_card(ui, &pal, |ui| {
                                    ui.label(
                                        egui::RichText::new("Feather Chat 1.0")
                                            .font(theme::semibold(13.0))
                                            .color(pal.text),
                                    );
                                    ui.add_space(3.0);
                                    ui.label(
                                        egui::RichText::new(
                                            "A deliberately light chat app: native, quiet, and quick to open. No account, no cloud, no tracking.",
                                        )
                                        .font(theme::font(11.5))
                                        .color(pal.weak),
                                    );
                                });
                                ui.add_space(44.0);
                            },
                        );
                    });
                });
            });
        if let Some(dark) = set_dark {
            self.set_dark(ctx, dark);
        }
    }

    fn contact_picker(&mut self, ctx: &egui::Context) {
        let pal = self.pal();
        let screen = ctx.screen_rect();
        let size = Vec2::new(430.0, 544.0);
        let pos = screen.center() - size * 0.5;

        // Dim the app behind the picker.
        let mut close = false;
        egui::Area::new(Id::new("picker_dim"))
            .order(egui::Order::Foreground)
            .fixed_pos(screen.min)
            .show(ctx, |ui| {
                let (rect, resp) = ui.allocate_exact_size(screen.size(), Sense::click());
                ui.painter().rect_filled(rect, 0.0, pal.overlay);
                if resp.clicked() {
                    close = true;
                }
            });

        let mut picked: Option<u64> = None;
        let mut call_pick: Option<(String, String, Color32)> = None;
        egui::Area::new(Id::new("picker"))
            .order(egui::Order::Foreground)
            .fixed_pos(pos)
            .show(ctx, |ui| {
                Frame::none()
                    .fill(pal.panel)
                    .rounding(16.0)
                    .shadow(egui::epaint::Shadow {
                        offset: Vec2::new(0.0, 10.0),
                        blur: 30.0,
                        spread: 0.0,
                        color: pal.shadow,
                    })
                    .show(ui, |ui| {
                        ui.set_width(size.x);
                        ui.add_space(16.0);
                        ui.horizontal(|ui| {
                            ui.add_space(20.0);
                            ui.label(
                                egui::RichText::new(if self.picker_for_call {
                                    "Start a call"
                                } else {
                                    "New chat"
                                })
                                .font(theme::semibold(18.0))
                                .color(pal.text),
                            );
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                ui.add_space(14.0);
                                if icon_button(ui, &pal, 32.0, "Close", |p, r, c| {
                                    icons::close(p, r, c, 1.4)
                                })
                                .clicked()
                                {
                                    close = true;
                                }
                            });
                        });
                        ui.add_space(12.0);
                        ui.horizontal(|ui| {
                            ui.add_space(18.0);
                            search_field(
                                ui,
                                &pal,
                                &mut self.picker_search,
                                "Search contacts",
                                size.x - 36.0,
                            );
                        });
                        ui.add_space(10.0);
                        divider_sized(ui, &pal, size.x - 36.0, 18.0);
                        ui.add_space(4.0);

                        ScrollArea::vertical()
                            .id_source("picker_scroll")
                            .max_height(390.0)
                            .auto_shrink([false, false])
                            .show(ui, |ui| {
                                let query = self.picker_search.trim().to_lowercase();
                                for chat in self.chats.iter() {
                                    if !query.is_empty()
                                        && !chat.name.to_lowercase().contains(&query)
                                    {
                                        continue;
                                    }
                                    ui.horizontal(|ui| {
                                        ui.add_space(14.0);
                                        let (row, resp) = ui.allocate_exact_size(
                                            Vec2::new(size.x - 28.0, 60.0),
                                            Sense::click(),
                                        );
                                        if resp.hovered() {
                                            ui.painter().rect_filled(row, 9.0, pal.hover);
                                        }
                                        let av = Rect::from_center_size(
                                            Pos2::new(row.left() + 28.0, row.center().y),
                                            Vec2::splat(40.0),
                                        );
                                        paint_avatar(
                                            ui.painter(),
                                            av,
                                            &chat.initials,
                                            chat.color,
                                            chat.online && !chat.is_group,
                                            &pal,
                                        );
                                        ui.painter().text(
                                            Pos2::new(row.left() + 60.0, row.top() + 12.0),
                                            Align2::LEFT_TOP,
                                            &chat.name,
                                            theme::semibold(13.5),
                                            pal.text,
                                        );
                                        ui.painter().text(
                                            Pos2::new(row.left() + 60.0, row.top() + 32.0),
                                            Align2::LEFT_TOP,
                                            if chat.is_group {
                                                format!("Group · {} members", chat.members)
                                            } else {
                                                chat.status_line()
                                            },
                                            theme::font(10.5),
                                            pal.weak,
                                        );
                                        if resp.clicked() {
                                            if self.picker_for_call {
                                                call_pick = Some((
                                                    chat.name.clone(),
                                                    chat.initials.clone(),
                                                    chat.color,
                                                ));
                                            } else {
                                                picked = Some(chat.id);
                                            }
                                        }
                                    });
                                }
                                ui.add_space(8.0);
                            });
                        ui.add_space(6.0);
                    });
            });

        if close {
            self.show_picker = false;
        }
        if let Some(id) = picked {
            self.open_chat(id);
        }
        if let Some((name, initials, color)) = call_pick {
            self.call = Some(ActiveCall {
                name,
                initials,
                color,
                video: false,
                started: self.now,
                muted: false,
                answered: None,
            });
            self.show_picker = false;
        }
    }

    fn call_overlay(&mut self, ctx: &egui::Context) {
        let pal = self.pal();
        let now = self.now;
        let Some(call) = self.call.as_mut() else {
            return;
        };
        if call.answered.is_none() && now - call.started > 2.6 {
            call.answered = Some(now);
        }
        let name = call.name.clone();
        let initials = call.initials.clone();
        let color = call.color;
        let video = call.video;
        let muted = call.muted;
        let answered = call.answered;
        let started = call.started;

        let screen = ctx.screen_rect();

        egui::Area::new(Id::new("call_dim"))
            .order(egui::Order::Foreground)
            .fixed_pos(screen.min)
            .show(ctx, |ui| {
                let (rect, _) = ui.allocate_exact_size(screen.size(), Sense::click());
                ui.painter().rect_filled(rect, 0.0, Color32::from_rgba_unmultiplied(6, 12, 16, 205));
            });

        let card = Vec2::new(380.0, 478.0);
        let pos = screen.center() - card * 0.5;
        let mut end = false;
        let mut toggle_mute = false;
        let mut toggle_video = false;
        egui::Area::new(Id::new("call_card"))
            .order(egui::Order::Foreground)
            .fixed_pos(pos)
            .show(ctx, |ui| {
                Frame::none()
                    .fill(Color32::from_rgb(25, 35, 41))
                    .rounding(22.0)
                    .shadow(egui::epaint::Shadow {
                        offset: Vec2::new(0.0, 16.0),
                        blur: 40.0,
                        spread: 0.0,
                        color: Color32::from_rgba_unmultiplied(0, 0, 0, 90),
                    })
                    .show(ui, |ui| {
                        ui.set_width(card.x);
                        ui.add_space(26.0);
                        ui.vertical_centered(|ui| {
                            if video {
                                let (r, _) =
                                    ui.allocate_exact_size(Vec2::new(320.0, 130.0), Sense::hover());
                                video_preview(ui.painter(), r, color);
                                ui.add_space(12.0);
                            } else {
                                let (av, _) =
                                    ui.allocate_exact_size(Vec2::splat(74.0), Sense::hover());
                                paint_avatar(ui.painter(), av, &initials, color, false, &pal);
                                ui.add_space(12.0);
                            }
                            ui.label(
                                egui::RichText::new(&name)
                                    .font(theme::semibold(21.0))
                                    .color(Color32::WHITE),
                            );
                            ui.add_space(3.0);
                            let status = if let Some(start) = answered {
                                let secs = (now - start).max(0.0) as u32;
                                format!("{}:{:02}", secs / 60, secs % 60)
                            } else {
                                "Ringing…".to_owned()
                            };
                            ui.label(
                                egui::RichText::new(status)
                                    .font(theme::font(12.0))
                                    .color(Color32::from_rgba_unmultiplied(230, 240, 238, 180)),
                            );
                            ui.add_space(24.0);
                            ui.horizontal(|ui| {
                                ui.add_space(82.0);
                                if call_button(
                                    ui,
                                    if muted {
                                        Color32::from_rgb(96, 110, 116)
                                    } else {
                                        Color32::from_rgb(54, 68, 74)
                                    },
                                    |p, r, c| {
                                        icons::mic(p, r, c, 1.7);
                                        if muted {
                                            p.line_segment(
                                                [
                                                    r.left_bottom() + Vec2::new(2.0, -1.0),
                                                    r.right_top() + Vec2::new(-2.0, 1.0),
                                                ],
                                                Stroke::new(1.8_f32, Color32::from_rgb(250, 160, 160)),
                                            );
                                        }
                                    },
                                )
                                .on_hover_text(if muted { "Unmute" } else { "Mute" })
                                .clicked()
                                {
                                    toggle_mute = true;
                                }
                                ui.add_space(14.0);
                                if call_button(ui, Color32::from_rgb(214, 64, 64), |p, r, c| {
                                    icons::phone(p, r, c, 1.6)
                                })
                                .on_hover_text("End call")
                                .clicked()
                                {
                                    end = true;
                                }
                                ui.add_space(14.0);
                                if call_button(ui, Color32::from_rgb(54, 68, 74), |p, r, c| {
                                    icons::video(p, r, c, 1.6)
                                })
                                .on_hover_text(if video { "Turn camera off" } else { "Turn camera on" })
                                .clicked()
                                {
                                    toggle_video = true;
                                }
                            });
                            ui.add_space(16.0);
                            ui.label(
                                egui::RichText::new(format!(
                                    "Feather call · {}",
                                    if video { "video" } else { "voice" }
                                ))
                                .font(theme::font(9.5))
                                .color(Color32::from_rgba_unmultiplied(230, 240, 238, 100)),
                            );
                        });
                    });
            });

        if toggle_mute {
            if let Some(call) = self.call.as_mut() {
                call.muted = !call.muted;
            }
        }
        if toggle_video {
            if let Some(call) = self.call.as_mut() {
                call.video = !call.video;
            }
        }
        if end {
            self.call = None;
            let secs = (now - started).max(0.0) as u32;
            self.toast(format!("Call ended · {}:{:02}", secs / 60, secs % 60));
        }
        if answered.is_none() {
            ctx.request_repaint_after(std::time::Duration::from_millis(300));
        }
    }

    fn popups(&mut self, ctx: &egui::Context) {
        let pal = self.pal();
        let screen = ctx.screen_rect();

        if self.show_menu {
            let armed = self.menu_armed;
            let pos = Pos2::new(screen.left() + 372.0, screen.top() + 58.0);
            let mut close = false;
            egui::Area::new(Id::new("menu"))
                .order(egui::Order::Tooltip)
                .fixed_pos(pos)
                .show(ctx, |ui| {
                    Frame::none()
                        .fill(pal.panel)
                        .rounding(12.0)
                        .shadow(egui::epaint::Shadow {
                            offset: Vec2::new(0.0, 6.0),
                            blur: 20.0,
                            spread: 0.0,
                            color: pal.shadow,
                        })
                        .inner_margin(Margin::symmetric(6.0, 6.0))
                        .show(ui, |ui| {
                            ui.set_width(206.0);
                            if menu_item(ui, &pal, "Mark all as read").clicked() {
                                for c in self.chats.iter_mut() {
                                    c.unread = 0;
                                }
                                close = true;
                            }
                            if menu_item(ui, &pal, "Settings").clicked() {
                                self.view = View::Settings;
                                close = true;
                            }
                            if menu_item(
                                ui,
                                &pal,
                                if self.dark { "Light theme" } else { "Dark theme" },
                            )
                            .clicked()
                            {
                                let target = !self.dark;
                                self.set_dark(ctx, target);
                                close = true;
                            }
                            if menu_item(ui, &pal, "About Feather").clicked() {
                                self.view = View::Settings;
                                close = true;
                                self.toast("Feather Chat 1.0 · made to stay small");
                            }
                            self.menu_rect = ui.min_rect();
                        });
                });
            self.menu_rect = self.menu_rect.expand(8.0);
            if close {
                self.show_menu = false;
            }
            if self.show_menu && armed && ctx.input(|i| i.pointer.any_click()) {
                if let Some(p) = ctx.input(|i| i.pointer.interact_pos()) {
                    if !self.menu_rect.contains(p) {
                        self.show_menu = false;
                    }
                }
            }
            self.menu_armed = true;
        }

        if self.popup != Popup::None {
            let armed = self.popup_armed;
            let anchor = Pos2::new(
                screen.left() + 424.0,
                screen.bottom() - 326.0,
            );
            let mut close = false;
            let mut appended: Option<String> = None;
            let mut action: Option<Body> = None;
            egui::Area::new(Id::new("composer_popup"))
                .order(egui::Order::Tooltip)
                .fixed_pos(anchor)
                .show(ctx, |ui| {
                    Frame::none()
                        .fill(pal.panel)
                        .rounding(14.0)
                        .shadow(egui::epaint::Shadow {
                            offset: Vec2::new(0.0, 8.0),
                            blur: 24.0,
                            spread: 0.0,
                            color: pal.shadow,
                        })
                        .inner_margin(Margin::symmetric(10.0, 10.0))
                        .show(ui, |ui| {
                            if self.popup == Popup::Emoji {
                                ui.set_width(276.0);
                                let emojis = [
                                    "😀", "😄", "😂", "😊", "😍", "😉", "🙂", "😅", "😢",
                                    "😮", "😴", "🤔", "👍", "👎", "👏", "🙏", "💪", "❤",
                                    "☕", "🔥", "✨", "🎉", "🌞", "🌙", "⭐", "📷", "🎧", "☎",
                                ];
                                egui::Grid::new("emoji_grid")
                                    .spacing(Vec2::new(2.0, 2.0))
                                    .show(ui, |ui| {
                                        for (i, e) in emojis.iter().enumerate() {
                                            let (r, resp) = ui.allocate_exact_size(
                                                Vec2::splat(31.0),
                                                Sense::click(),
                                            );
                                            if resp.hovered() {
                                                ui.painter().rect_filled(
                                                    r,
                                                    6.0,
                                                    pal.hover,
                                                );
                                            }
                                            ui.painter().text(
                                                r.center(),
                                                Align2::CENTER_CENTER,
                                                *e,
                                                theme::font(18.0),
                                                pal.text,
                                            );
                                            if resp.clicked() {
                                                appended = Some((*e).to_owned());
                                            }
                                            if (i + 1) % 7 == 0 {
                                                ui.end_row();
                                            }
                                        }
                                    });
                            } else {
                                ui.set_width(218.0);
                                if attach_item(ui, &pal, "Photo", |p, r, c| {
                                    icons::photo(p, r, c, 1.4)
                                })
                                .clicked()
                                {
                                    action = Some(Body::Photo {
                                        caption: String::new(),
                                        tint: Color32::from_rgb(111, 158, 178),
                                    });
                                }
                                if attach_item(ui, &pal, "Document", |p, r, c| {
                                    icons::document(p, r, c, 1.4)
                                })
                                .clicked()
                                {
                                    action = Some(Body::Document {
                                        name: "design-notes.pdf".to_owned(),
                                        size: "240 KB".to_owned(),
                                    });
                                }
                                if attach_item(ui, &pal, "Location", |p, r, c| {
                                    icons::location(p, r, c, 1.4)
                                })
                                .clicked()
                                {
                                    action = Some(Body::Location {
                                        place: "Elm Street Café".to_owned(),
                                    });
                                }
                            }
                            self.popup_rect = ui.min_rect().expand(10.0);
                        });
                });

            if let Some(e) = appended {
                if let Some(chat) = self.chats.iter_mut().find(|c| c.id == self.selected) {
                    chat.draft.push_str(&e);
                }
                close = true;
                self.focus_composer = true;
            }
            if let Some(body) = action {
                self.push_outgoing(body);
                close = true;
            }
            if close {
                self.popup = Popup::None;
            }
            if self.popup != Popup::None && armed && ctx.input(|i| i.pointer.any_click()) {
                if let Some(p) = ctx.input(|i| i.pointer.interact_pos()) {
                    if !self.popup_rect.contains(p) {
                        self.popup = Popup::None;
                    }
                }
            }
            self.popup_armed = true;
        }

        if let Some(t) = &self.toast {
            let alpha = ((t.until - self.now) / 0.5).clamp(0.0, 1.0);
            let fill = Color32::from_rgba_unmultiplied(22, 32, 38, (230.0 * alpha) as u8);
            egui::Area::new(Id::new("toast"))
                .order(egui::Order::Tooltip)
                .anchor(Align2::CENTER_BOTTOM, Vec2::new(0.0, -108.0))
                .show(ctx, |ui| {
                    Frame::none()
                        .fill(fill)
                        .rounding(10.0)
                        .inner_margin(Margin::symmetric(18.0, 10.0))
                        .show(ui, |ui| {
                            ui.label(
                                egui::RichText::new(&t.text)
                                    .font(theme::font(12.0))
                                    .color(Color32::from_rgba_unmultiplied(
                                        255,
                                        255,
                                        255,
                                        (255.0 * alpha) as u8,
                                    )),
                            );
                        });
                });
        }
    }
}

impl eframe::App for FeatherApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.handle_screenshot(ctx);
        self.now = ctx.input(|i| i.time);
        self.process_time(ctx);
        self.handle_escape(ctx);

        self.rail(ctx);
        let pal = self.pal();

        match self.view {
            View::Chats => {
                if self.link.connected() {
                    // Once a space is linked, the main Chats tab is the real
                    // conversation list. The sample chats come back only after
                    // disconnecting.
                    self.link.ui(ctx, &pal);
                } else {
                    self.chat_list_panel(ctx);
                    if self.show_info {
                        self.info_panel(ctx);
                    }
                    self.chat_header(ctx);
                    self.composer(ctx);
                    self.thread_panel(ctx);
                }
            }
            View::Calls => self.calls_panel(ctx),
            View::Link => self.link.ui(ctx, &pal),
            View::Settings => self.settings_panel(ctx),
        }

        if self.show_picker {
            self.contact_picker(ctx);
        }
        if self.call.is_some() {
            self.call_overlay(ctx);
        }
        self.popups(ctx);
    }
}

// ---------- small painting helpers ----------

struct LocalNow;
impl LocalNow {
    fn time() -> String {
        chrono::Local::now().format("%H:%M").to_string()
    }
}

trait ShallowClone {
    fn clone_shallow(&self) -> Chat;
}

impl ShallowClone for Chat {
    fn clone_shallow(&self) -> Chat {
        Chat {
            id: self.id,
            name: self.name.clone(),
            initials: self.initials.clone(),
            color: self.color,
            messages: self.messages.clone(),
            pinned: self.pinned,
            muted: self.muted,
            unread: self.unread,
            online: self.online,
            last_seen: self.last_seen.clone(),
            about: self.about.clone(),
            phone: self.phone.clone(),
            is_group: self.is_group,
            members: self.members,
            typing: self.typing,
            draft: self.draft.clone(),
            updated: self.updated,
        }
    }
}

fn reply_lines(chat: &Chat) -> &'static [&'static str] {
    match chat.id {
        1 => &[
            "Perfect — give me five minutes.",
            "That makes sense to me.",
            "Ha, fair. Let's do it.",
            "Just checked. Looks good on my side.",
        ],
        2 => &[
            "Works for me!",
            "Adding it to the group plan.",
            "I'll bring an extra flask.",
        ],
        4 => &[
            "Lovely, talk in a bit ❤",
            "Don't forget to eat something.",
            "Call me when you can, no rush.",
        ],
        _ => &[
            "Got it!",
            "Sounds good to me.",
            "On it — talk in a bit.",
            "Thanks for the note.",
        ],
    }
}

fn paint_avatar(
    painter: &egui::Painter,
    rect: Rect,
    initials: &str,
    color: Color32,
    online: bool,
    pal: &Palette,
) {
    let r = rect.width() * 0.5;
    painter.circle_filled(rect.center(), r, color);
    painter.circle_filled(
        rect.center() - Vec2::new(0.0, r * 0.06),
        r * 0.96,
        Color32::from_rgba_unmultiplied(255, 255, 255, 10),
    );
    painter.text(
        rect.center(),
        Align2::CENTER_CENTER,
        initials,
        theme::semibold(r * 0.72),
        Color32::from_rgb(255, 255, 255),
    );
    if online {
        let dot = rect.center() + Vec2::new(r * 0.66, r * 0.66);
        painter.circle_filled(dot, r * 0.22, pal.panel);
        painter.circle_filled(dot, r * 0.15, pal.accent);
    }
}

fn icon_button(
    ui: &mut Ui,
    pal: &Palette,
    size: f32,
    tooltip: &str,
    draw: impl Fn(&egui::Painter, Rect, Color32),
) -> Response {
    let (rect, resp) = ui.allocate_exact_size(Vec2::splat(size), Sense::click());
    if resp.hovered() {
        ui.painter().circle_filled(rect.center(), size * 0.5, pal.hover);
    }
    draw(ui.painter(), rect.shrink(size * 0.27), pal.weak);
    resp.on_hover_text(tooltip)
        .on_hover_cursor(egui::CursorIcon::PointingHand)
}

fn rail_button(
    ui: &mut Ui,
    pal: &Palette,
    active: bool,
    tooltip: &str,
    draw: impl Fn(&egui::Painter, Rect, Color32),
) -> Response {
    let (rect, resp) = ui.allocate_exact_size(Vec2::splat(46.0), Sense::click());
    if active {
        ui.painter().rect_filled(rect, 13.0, pal.accent_soft);
    } else if resp.hovered() {
        ui.painter().rect_filled(rect, 13.0, pal.hover);
    }
    draw(
        ui.painter(),
        rect.shrink(13.0),
        if active { pal.accent } else { pal.weak },
    );
    resp.on_hover_text(tooltip)
        .on_hover_cursor(egui::CursorIcon::PointingHand)
}

fn search_field(ui: &mut Ui, pal: &Palette, text: &mut String, hint: &str, width: f32) -> Response {
    let mut resp = None;
    Frame::none()
        .fill(pal.search_bg)
        .rounding(10.0)
        .inner_margin(Margin::symmetric(11.0, 8.0))
        .show(ui, |ui| {
            ui.set_width(width - 22.0);
            ui.horizontal(|ui| {
                let (r, _) = ui.allocate_exact_size(Vec2::splat(17.0), Sense::hover());
                icons::search(ui.painter(), r, pal.weak, 1.4);
                ui.add_space(2.0);
                let edit = ui.add(
                    TextEdit::singleline(text)
                        .frame(false)
                        .hint_text(hint)
                        .font(theme::font(12.5))
                        .text_color(pal.text)
                        .desired_width(f32::INFINITY),
                );
                resp = Some(edit);
            });
        });
    resp.unwrap_or_else(|| ui.allocate_response(Vec2::ZERO, Sense::hover()))
}

fn chip(ui: &mut Ui, pal: &Palette, label: &str, active: bool) -> Response {
    let galley = ui
        .painter()
        .layout_no_wrap(label.to_owned(), theme::font(11.5), pal.text);
    let w = galley.size().x + 24.0;
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, 26.0), Sense::click());
    let fill = if active {
        pal.accent_soft
    } else if resp.hovered() {
        pal.hover
    } else {
        pal.panel_alt
    };
    ui.painter().rect_filled(rect, 13.0, fill);
    ui.painter().text(
        rect.center(),
        Align2::CENTER_CENTER,
        label,
        theme::font(11.5),
        if active { pal.accent_deep } else { pal.weak },
    );
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
}

fn sender_color(name: &str) -> Color32 {
    const COLORS: [Color32; 6] = [
        Color32::from_rgb(214, 126, 96),
        Color32::from_rgb(96, 156, 110),
        Color32::from_rgb(96, 132, 196),
        Color32::from_rgb(157, 116, 196),
        Color32::from_rgb(198, 146, 82),
        Color32::from_rgb(84, 150, 154),
    ];
    let hash = name
        .bytes()
        .fold(0usize, |acc, b| acc.wrapping_mul(31).wrapping_add(b as usize));
    COLORS[hash % COLORS.len()]
}

fn ellipsize(ui: &Ui, text: &str, font: FontId, max_w: f32) -> String {
    let measure = |s: &str| {
        ui.painter()
            .layout_no_wrap(s.to_owned(), font.clone(), Color32::WHITE)
            .size()
            .x
    };
    if measure(text) <= max_w {
        return text.to_owned();
    }
    let chars: Vec<char> = text.chars().collect();
    if chars.is_empty() {
        return String::new();
    }
    let mut lo = 0usize;
    let mut hi = chars.len();
    while lo < hi {
        let mid = (lo + hi + 1) / 2;
        let mut candidate: String = chars[..mid].iter().collect();
        candidate.push('…');
        if measure(&candidate) <= max_w {
            lo = mid;
        } else {
            hi = mid - 1;
        }
    }
    let mut out: String = chars[..lo].iter().collect();
    out.push('…');
    out
}

fn lite_badge(ui: &mut Ui, pal: &Palette) {    let (r, _) = ui.allocate_exact_size(Vec2::new(30.0, 15.0), Sense::hover());
    ui.painter().rect_filled(r, 5.0, pal.accent_soft);
    ui.painter().text(
        r.center(),
        Align2::CENTER_CENTER,
        "LITE",
        theme::semibold(7.5),
        pal.accent_deep,
    );
}

fn paint_wallpaper(painter: &egui::Painter, rect: Rect, pal: &Palette) {
    painter.rect_filled(rect, 0.0, pal.wallpaper);
    let ink = if pal.dark {
        Color32::from_rgba_unmultiplied(255, 255, 255, 4)
    } else {
        Color32::from_rgba_unmultiplied(112, 96, 72, 12)
    };
    let accent_ink = if pal.dark {
        Color32::from_rgba_unmultiplied(0, 168, 132, 8)
    } else {
        Color32::from_rgba_unmultiplied(70, 130, 104, 12)
    };
    let cell = 94.0;
    let mut y = rect.top() + 26.0;
    let mut row = 0;
    while y < rect.bottom() {
        let mut x = rect.left() + 30.0;
        let mut col = 0;
        while x < rect.right() {
            let kind = (row * 5 + col * 3) % 7;
            let c = match kind {
                0 | 4 => accent_ink,
                _ => ink,
            };
            match kind {
                0 => {
                    painter.circle_stroke(
                        Pos2::new(x, y),
                        6.0,
                        Stroke::new(1.0, c),
                    );
                }
                1 => {
                    painter.line_segment(
                        [Pos2::new(x - 5.0, y + 5.0), Pos2::new(x + 5.0, y - 5.0)],
                        Stroke::new(1.0, c),
                    );
                    painter.line_segment(
                        [Pos2::new(x - 5.0, y - 5.0), Pos2::new(x + 5.0, y + 5.0)],
                        Stroke::new(1.0, c),
                    );
                }
                2 => {
                    painter.add(egui::Shape::line(
                        vec![
                            Pos2::new(x - 7.0, y),
                            Pos2::new(x, y - 6.0),
                            Pos2::new(x + 7.0, y),
                            Pos2::new(x, y + 6.0),
                        ],
                        Stroke::new(1.0, c),
                    ));
                }
                3 => {
                    painter.add(egui::Shape::line(
                        vec![
                            Pos2::new(x - 6.0, y + 4.0),
                            Pos2::new(x - 2.0, y - 4.0),
                            Pos2::new(x + 2.0, y + 3.0),
                            Pos2::new(x + 6.0, y - 4.0),
                        ],
                        Stroke::new(1.0, c),
                    ));
                }
                5 => {
                    painter.line_segment(
                        [Pos2::new(x - 6.0, y), Pos2::new(x + 6.0, y)],
                        Stroke::new(1.0, c),
                    );
                    painter.line_segment(
                        [Pos2::new(x, y - 6.0), Pos2::new(x, y + 6.0)],
                        Stroke::new(1.0, c),
                    );
                }
                _ => {
                    painter.add(egui::Shape::line(
                        vec![
                            Pos2::new(x - 3.0, y + 5.0),
                            Pos2::new(x - 3.0, y - 3.0),
                            Pos2::new(x + 5.0, y - 3.0),
                        ],
                        Stroke::new(1.0, c),
                    ));
                }
            }
            x += cell;
            col += 1;
        }
        y += cell;
        row += 1;
    }
}

fn day_pill(ui: &mut Ui, pal: &Palette, day: &str) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 34.0), Sense::hover());
    let label = day.to_uppercase();
    let galley = ui.painter().layout_no_wrap(
        label,
        theme::semibold(9.0),
        pal.weak,
    );
    let pill = Rect::from_center_size(
        rect.center(),
        Vec2::new(galley.size().x + 24.0, 21.0),
    );
    ui.painter().rect_filled(
        pill,
        7.0,
        Color32::from_rgba_unmultiplied(pal.panel.r(), pal.panel.g(), pal.panel.b(), 235),
    );
    ui.painter().galley(
        Pos2::new(
            pill.center().x - galley.size().x * 0.5,
            pill.center().y - galley.size().y * 0.5,
        ),
        galley,
        pal.weak,
    );
}

fn message_bubble(ui: &mut Ui, pal: &Palette, msg: &Message, max_w: f32, tight: bool) {
    let out = msg.out;
    let radius = 13.0;
    let rounding = if out {
        Rounding {
            nw: radius,
            ne: radius,
            sw: radius,
            se: 4.0,
        }
    } else {
        Rounding {
            nw: radius,
            ne: radius,
            sw: 4.0,
            se: radius,
        }
    };

    let (w, h, text_galley) = match &msg.body {
        Body::Text(text) => {
            let galley = ui.fonts(|f| {
                f.layout(
                    text.clone(),
                    theme::font(14.0),
                    pal.text,
                    max_w - 26.0,
                )
            });
            let w = (galley.size().x + 26.0).max(86.0);
            let h = galley.size().y + 31.0;
            (w, h, Some(galley))
        }
        Body::Photo { .. } => (240.0, 194.0, None),
        Body::Document { .. } => (250.0, 78.0, None),
        Body::Voice { .. } => (232.0, 72.0, None),
        Body::Location { .. } => (226.0, 152.0, None),
    };

    let w = w.min(max_w);
    let margin = 20.0;

    ui.with_layout(Layout::top_down(Align::Min), |ui| {
        let sender = if out {
            None
        } else {
            msg.sender.as_deref()
        };
        let sender_h = if sender.is_some() { 16.0 } else { 0.0 };
        let (row, _) = ui.allocate_exact_size(
            Vec2::new(ui.available_width(), h + sender_h),
            Sense::hover(),
        );
        let x = if out {
            row.right() - margin - w
        } else {
            row.left() + margin
        };
        if let Some(name) = sender {
            ui.painter().text(
                Pos2::new(x, row.top()),
                Align2::LEFT_TOP,
                name,
                theme::semibold(10.5),
                sender_color(name),
            );
        }
        let rect = Rect::from_min_size(
            Pos2::new(x, row.top() + sender_h),
            Vec2::new(w, h),
        );
        let painter = ui.painter();
        painter.rect_filled(
            Rect::from_min_size(rect.min + Vec2::new(0.0, 1.5), rect.size()),
            rounding,
            Color32::from_rgba_unmultiplied(0, 0, 0, 14),
        );
        painter.rect_filled(
            rect,
            rounding,
            if out { pal.outgoing } else { pal.incoming },
        );

        match &msg.body {
            Body::Text(text) => {
                if let Some(galley) = text_galley {
                    painter.galley(
                        Pos2::new(rect.left() + 13.0, rect.top() + 8.0),
                        galley,
                        pal.text,
                    );
                } else {
                    painter.text(
                        Pos2::new(rect.left() + 13.0, rect.top() + 9.0),
                        Align2::LEFT_TOP,
                        text,
                        theme::font(14.0),
                        pal.text,
                    );
                }
            }
            Body::Photo { caption, tint } => {
                let img = Rect::from_min_size(
                    rect.min + Vec2::new(4.0, 4.0),
                    Vec2::new(rect.width() - 8.0, 140.0),
                );
                photo_placeholder(painter, img, *tint);
                if !caption.is_empty() {
                    painter.text(
                        Pos2::new(rect.left() + 12.0, img.bottom() + 7.0),
                        Align2::LEFT_TOP,
                        caption,
                        theme::font(12.0),
                        pal.text,
                    );
                }
            }
            Body::Document { name, size } => {
                let icon = Rect::from_min_size(
                    rect.min + Vec2::new(12.0, 14.0),
                    Vec2::splat(36.0),
                );
                painter.circle_filled(icon.center(), 18.0, pal.accent_soft);
                icons::document(
                    painter,
                    icon.shrink(9.0),
                    pal.accent_deep,
                    1.4,
                );
                let clipped = painter.with_clip_rect(Rect::from_min_max(
                    Pos2::new(rect.left() + 56.0, rect.top()),
                    Pos2::new(rect.right() - 10.0, rect.bottom()),
                ));
                clipped.text(
                    Pos2::new(rect.left() + 56.0, rect.top() + 14.0),
                    Align2::LEFT_TOP,
                    name,
                    theme::semibold(12.5),
                    pal.text,
                );
                clipped.text(
                    Pos2::new(rect.left() + 56.0, rect.top() + 33.0),
                    Align2::LEFT_TOP,
                    size,
                    theme::font(10.5),
                    pal.weak,
                );
            }
            Body::Voice { secs } => {
                let play = Rect::from_min_size(
                    rect.min + Vec2::new(12.0, 12.0),
                    Vec2::splat(34.0),
                );
                painter.circle_filled(play.center(), 17.0, pal.accent);
                let tri = vec![
                    Pos2::new(play.center().x - 3.0, play.center().y - 6.0),
                    Pos2::new(play.center().x + 7.0, play.center().y),
                    Pos2::new(play.center().x - 3.0, play.center().y + 6.0),
                ];
                painter.add(egui::Shape::convex_polygon(
                    tri,
                    Color32::WHITE,
                    Stroke::NONE,
                ));
                let bars_left = rect.left() + 56.0;
                let mid = rect.top() + 29.0;
                for i in 0..18 {
                    let bh = 4.0 + ((i * 7 + 3) % 11) as f32;
                    let x = bars_left + i as f32 * 5.0;
                    painter.line_segment(
                        [
                            Pos2::new(x, mid - bh * 0.5),
                            Pos2::new(x, mid + bh * 0.5),
                        ],
                        Stroke::new(2.0, pal.weak),
                    );
                }
                painter.text(
                    Pos2::new(rect.right() - 10.0, rect.top() + 23.0),
                    Align2::RIGHT_TOP,
                    format!("{}:{:02}", secs / 60, secs % 60),
                    theme::font(9.5),
                    pal.weak,
                );
            }
            Body::Location { place } => {
                let map = Rect::from_min_size(
                    rect.min + Vec2::new(4.0, 4.0),
                    Vec2::new(rect.width() - 8.0, 100.0),
                );
                map_placeholder(painter, map, pal);
                painter.text(
                    Pos2::new(rect.left() + 12.0, map.bottom() + 8.0),
                    Align2::LEFT_TOP,
                    place,
                    theme::font(12.0),
                    pal.text,
                );
            }
        }

        // Time + status ticks
        let time_font = theme::font(9.5);
        let time_galley =
            painter.layout_no_wrap(msg.time.clone(), time_font, pal.weak);
        let tw = time_galley.size().x;
        let meta_y = rect.bottom() - 8.0;
        let time_pos = Pos2::new(rect.right() - 10.0 - tw, meta_y - 9.0);
        let meta_color = if out { pal.weak } else { pal.weak };
        painter.galley(time_pos, time_galley, meta_color);

        if out {
            let tick_rect = Rect::from_min_size(
                Pos2::new(rect.right() - 12.0 - tw - 18.0, meta_y - 12.0),
                Vec2::new(16.0, 12.0),
            );
            match msg.status {
                Status::Sending => icons::clock(painter, tick_rect, pal.weak, 1.2),
                Status::Sent => icons::ticks(painter, tick_rect, pal.weak, 1.3, false),
                Status::Delivered => icons::ticks(painter, tick_rect, pal.weak, 1.3, true),
                Status::Read => icons::ticks(painter, tick_rect, pal.tick, 1.4, true),
            }
        }

        let _ = tight;
    });
}

fn photo_placeholder(painter: &egui::Painter, rect: Rect, tint: Color32) {
    painter.rect_filled(rect, 9.0, tint);
    let lighter = lighten(tint, 26);
    painter.add(egui::Shape::convex_polygon(
        vec![
            Pos2::new(rect.left(), rect.bottom()),
            Pos2::new(rect.left(), rect.top() + rect.height() * 0.55),
            Pos2::new(rect.right() - rect.width() * 0.28, rect.top() + rect.height() * 0.28),
            Pos2::new(rect.right(), rect.top() + rect.height() * 0.5),
            Pos2::new(rect.right(), rect.bottom()),
        ],
        Color32::from_rgba_unmultiplied(lighter.r(), lighter.g(), lighter.b(), 160),
        Stroke::NONE,
    ));
    painter.circle_filled(
        Pos2::new(rect.left() + rect.width() * 0.74, rect.top() + rect.height() * 0.28),
        rect.height() * 0.10,
        Color32::from_rgba_unmultiplied(255, 250, 220, 190),
    );
    painter.add(egui::Shape::convex_polygon(
        vec![
            Pos2::new(rect.left(), rect.bottom()),
            Pos2::new(rect.left() + rect.width() * 0.30, rect.top() + rect.height() * 0.62),
            Pos2::new(rect.left() + rect.width() * 0.52, rect.bottom()),
        ],
        Color32::from_rgba_unmultiplied(30, 55, 50, 70),
        Stroke::NONE,
    ));
    painter.add(egui::Shape::convex_polygon(
        vec![
            Pos2::new(rect.left() + rect.width() * 0.32, rect.bottom()),
            Pos2::new(rect.left() + rect.width() * 0.64, rect.top() + rect.height() * 0.55),
            Pos2::new(rect.right(), rect.top() + rect.height() * 0.78),
            Pos2::new(rect.right(), rect.bottom()),
        ],
        Color32::from_rgba_unmultiplied(20, 45, 42, 58),
        Stroke::NONE,
    ));
}

fn map_placeholder(painter: &egui::Painter, rect: Rect, pal: &Palette) {
    painter.rect_filled(rect, 9.0, if pal.dark {
        Color32::from_rgb(34, 52, 48)
    } else {
        Color32::from_rgb(220, 232, 218)
    });
    let road = if pal.dark {
        Color32::from_rgb(52, 74, 68)
    } else {
        Color32::from_rgb(246, 249, 242)
    };
    for i in 0..4 {
        let y = rect.top() + rect.height() * (0.2 + i as f32 * 0.22);
        painter.line_segment(
            [
                Pos2::new(rect.left(), y + 8.0),
                Pos2::new(rect.right(), y - 6.0),
            ],
            Stroke::new(7.0, road),
        );
    }
    painter.line_segment(
        [
            Pos2::new(rect.left() + rect.width() * 0.4, rect.top()),
            Pos2::new(rect.left() + rect.width() * 0.55, rect.bottom()),
        ],
        Stroke::new(6.0, road),
    );
    let pin_rect = Rect::from_center_size(
        rect.center() + Vec2::new(0.0, -4.0),
        Vec2::splat(28.0),
    );
    icons::location(painter, pin_rect, pal.accent_deep, 1.8);
}

fn typing_bubble(ui: &mut Ui, pal: &Palette, time: f64) {
    let (row, _) = ui.allocate_exact_size(
        Vec2::new(ui.available_width(), 34.0),
        Sense::hover(),
    );
    let rect = Rect::from_min_size(row.min + Vec2::new(20.0, 0.0), Vec2::new(58.0, 34.0));
    ui.painter().rect_filled(rect, 13.0, pal.incoming);
    for i in 0..3 {
        let phase = (time * 3.2 + i as f64 * 0.55).sin() * 0.5 + 0.5;
        let r = 2.4 + phase as f32 * 1.4;
        let c = if pal.dark {
            Color32::from_rgba_unmultiplied(180, 200, 194, (130.0 + phase as f32 * 90.0) as u8)
        } else {
            Color32::from_rgba_unmultiplied(110, 130, 124, (110.0 + phase as f32 * 100.0) as u8)
        };
        ui.painter().circle_filled(
            Pos2::new(rect.left() + 17.0 + i as f32 * 12.0, rect.center().y),
            r,
            c,
        );
    }
    ui.add_space(8.0);
}

fn info_action_card(
    ui: &mut Ui,
    pal: &Palette,
    title: &str,
    subtitle: &str,
    draw: impl Fn(&egui::Painter, Rect, Color32),
    width: f32,
) -> Response {
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(width, 62.0), Sense::click());
    let fill = if resp.hovered() { pal.hover } else { pal.panel_alt };
    ui.painter().rect_filled(rect, 10.0, fill);
    draw(
        ui.painter(),
        Rect::from_min_size(rect.min + Vec2::new(11.0, 19.0), Vec2::splat(24.0)),
        pal.accent,
    );
    let clipped = ui.painter().with_clip_rect(rect.shrink(6.0));
    clipped.text(
        Pos2::new(rect.left() + 44.0, rect.top() + 16.0),
        Align2::LEFT_TOP,
        title,
        theme::semibold(11.5),
        pal.text,
    );
    clipped.text(
        Pos2::new(rect.left() + 44.0, rect.top() + 34.0),
        Align2::LEFT_TOP,
        subtitle,
        theme::font(9.0),
        pal.weak,
    );
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
}

fn media_thumb(painter: &egui::Painter, rect: Rect, pal: &Palette, kind: usize) {
    let (a, b) = match kind {
        0 => (
            Color32::from_rgb(152, 184, 168),
            Color32::from_rgb(104, 140, 132),
        ),
        1 => (
            Color32::from_rgb(196, 164, 130),
            Color32::from_rgb(146, 114, 92),
        ),
        _ => (
            Color32::from_rgb(140, 156, 188),
            Color32::from_rgb(96, 112, 148),
        ),
    };
    painter.rect_filled(rect, 8.0, a);
    painter.add(egui::Shape::convex_polygon(
        vec![
            Pos2::new(rect.left(), rect.bottom()),
            Pos2::new(rect.left() + rect.width() * 0.42, rect.top() + rect.height() * 0.44),
            Pos2::new(rect.right() - rect.width() * 0.12, rect.top() + rect.height() * 0.66),
            Pos2::new(rect.right(), rect.top() + rect.height() * 0.52),
            Pos2::new(rect.right(), rect.bottom()),
        ],
        b,
        Stroke::NONE,
    ));
    painter.circle_filled(
        Pos2::new(rect.left() + rect.width() * 0.68, rect.top() + rect.height() * 0.28),
        rect.width() * 0.09,
        Color32::from_rgba_unmultiplied(255, 248, 220, 180),
    );
    let _ = pal;
}

fn danger_button(ui: &mut Ui, pal: &Palette, label: &str) -> Response {
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(140.0, 36.0), Sense::click());
    let fill = if resp.hovered() {
        Color32::from_rgba_unmultiplied(pal.danger.r(), pal.danger.g(), pal.danger.b(), 22)
    } else {
        Color32::TRANSPARENT
    };
    ui.painter().rect_filled(rect, 8.0, fill);
    ui.painter().text(
        rect.center(),
        Align2::CENTER_CENTER,
        label,
        theme::font(12.0),
        pal.danger,
    );
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
}

fn accent_button(ui: &mut Ui, pal: &Palette, label: &str) -> Response {
    let galley = ui
        .painter()
        .layout_no_wrap(label.to_owned(), theme::semibold(12.0), Color32::WHITE);
    let w = galley.size().x + 32.0;
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, 36.0), Sense::click());
    ui.painter().rect_filled(
        rect,
        9.0,
        if resp.hovered() { pal.accent_deep } else { pal.accent },
    );
    ui.painter().text(
        rect.center(),
        Align2::CENTER_CENTER,
        label,
        theme::semibold(12.0),
        Color32::WHITE,
    );
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
}

fn section_title(ui: &mut Ui, pal: &Palette, text: &str) {
    ui.label(
        egui::RichText::new(text.to_uppercase())
            .font(theme::semibold(9.5))
            .color(pal.weak),
    );
}

fn settings_card(ui: &mut Ui, pal: &Palette, add: impl FnOnce(&mut Ui)) {
    Frame::none()
        .fill(pal.panel_alt)
        .rounding(13.0)
        .inner_margin(16.0)
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            add(ui);
        });
}

fn divider(ui: &mut Ui, pal: &Palette) {
    ui.add_space(8.0);
    let (r, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 1.0), Sense::hover());
    ui.painter().rect_filled(r, 0.0, pal.border);
    ui.add_space(8.0);
}

fn divider_sized(ui: &mut Ui, pal: &Palette, width: f32, pad: f32) {
    ui.horizontal(|ui| {
        ui.add_space(pad);
        let (r, _) = ui.allocate_exact_size(Vec2::new(width, 1.0), Sense::hover());
        ui.painter().rect_filled(r, 0.0, pal.border);
    });
}

fn toggle_row(ui: &mut Ui, pal: &Palette, title: &str, subtitle: &str, value: &mut bool) {
    let (rect, resp) = ui.allocate_exact_size(
        Vec2::new(ui.available_width(), 44.0),
        Sense::click(),
    );
    let mut clicked = resp.clicked();
    ui.painter().text(
        Pos2::new(rect.left(), rect.top() + 2.0),
        Align2::LEFT_TOP,
        title,
        theme::semibold(12.5),
        pal.text,
    );
    ui.painter().text(
        Pos2::new(rect.left(), rect.top() + 20.0),
        Align2::LEFT_TOP,
        subtitle,
        theme::font(10.0),
        pal.weak,
    );
    let switch = Rect::from_center_size(
        Pos2::new(rect.right() - 22.0, rect.center().y),
        Vec2::new(40.0, 22.0),
    );
    let sresp = ui.interact(
        switch,
        Id::new(("toggle", title)),
        Sense::click(),
    );
    if sresp.clicked() {
        clicked = true;
    }
    ui.painter().rect_filled(
        switch,
        11.0,
        if *value { pal.accent } else { pal.border },
    );
    let knob_x = if *value {
        switch.right() - 11.0
    } else {
        switch.left() + 11.0
    };
    ui.painter().circle_filled(
        Pos2::new(knob_x, switch.center().y),
        8.0,
        Color32::WHITE,
    );
    if clicked {
        *value = !*value;
    }
}

fn theme_tile(ui: &mut Ui, pal: &Palette, dark: bool, selected: bool) -> Response {
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(168.0, 92.0), Sense::click());
    let bg = if dark {
        Color32::from_rgb(17, 27, 33)
    } else {
        Color32::from_rgb(247, 248, 250)
    };
    ui.painter().rect_filled(rect, 10.0, bg);
    ui.painter().rect_stroke(
        rect,
        10.0,
        Stroke::new(
            if selected { 2.0 } else { 1.0 },
            if selected { pal.accent } else { pal.border },
        ),
    );
    let side = Rect::from_min_size(
        rect.min + Vec2::new(8.0, 8.0),
        Vec2::new(46.0, rect.height() - 16.0),
    );
    ui.painter().rect_filled(
        side,
        6.0,
        if dark {
            Color32::from_rgb(31, 44, 51)
        } else {
            Color32::from_rgb(255, 255, 255)
        },
    );
    let bubble = Rect::from_min_size(
        Pos2::new(rect.left() + 68.0, rect.top() + 26.0),
        Vec2::new(76.0, 22.0),
    );
    ui.painter().rect_filled(
        bubble,
        7.0,
        if dark {
            Color32::from_rgb(0, 92, 75)
        } else {
            Color32::from_rgb(221, 248, 228)
        },
    );
    let bubble2 = Rect::from_min_size(
        Pos2::new(rect.left() + 82.0, rect.top() + 55.0),
        Vec2::new(66.0, 18.0),
    );
    ui.painter().rect_filled(
        bubble2,
        7.0,
        if dark {
            Color32::from_rgb(32, 44, 51)
        } else {
            Color32::from_rgb(255, 255, 255)
        },
    );
    ui.painter().text(
        Pos2::new(rect.left() + 70.0, rect.top() + 7.0),
        Align2::LEFT_TOP,
        if dark {
            "Dark"
        } else {
            "Light"
        },
        theme::semibold(10.0),
        if dark {
            Color32::WHITE
        } else {
            Color32::from_rgb(30, 42, 48)
        },
    );
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
}

fn menu_item(ui: &mut Ui, pal: &Palette, label: &str) -> Response {
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 34.0), Sense::click());
    if resp.hovered() {
        ui.painter().rect_filled(rect, 7.0, pal.hover);
    }
    ui.painter().text(
        Pos2::new(rect.left() + 10.0, rect.center().y),
        Align2::LEFT_CENTER,
        label,
        theme::font(12.0),
        pal.text,
    );
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
}

fn attach_item(
    ui: &mut Ui,
    pal: &Palette,
    label: &str,
    draw: impl Fn(&egui::Painter, Rect, Color32),
) -> Response {
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 42.0), Sense::click());
    if resp.hovered() {
        ui.painter().rect_filled(rect, 8.0, pal.hover);
    }
    draw(
        ui.painter(),
        Rect::from_min_size(rect.min + Vec2::new(8.0, 12.0), Vec2::splat(19.0)),
        pal.accent,
    );
    ui.painter().text(
        Pos2::new(rect.left() + 38.0, rect.center().y),
        Align2::LEFT_CENTER,
        label,
        theme::font(12.0),
        pal.text,
    );
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
}

fn video_preview(painter: &egui::Painter, rect: Rect, color: Color32) {
    painter.rect_filled(rect, 14.0, darken(color, 40));
    painter.add(egui::Shape::convex_polygon(
        vec![
            Pos2::new(rect.left(), rect.bottom()),
            Pos2::new(rect.left() + rect.width() * 0.35, rect.top() + rect.height() * 0.45),
            Pos2::new(rect.right() - rect.width() * 0.18, rect.top() + rect.height() * 0.72),
            Pos2::new(rect.right(), rect.top() + rect.height() * 0.58),
            Pos2::new(rect.right(), rect.bottom()),
        ],
        darken(color, 62),
        Stroke::NONE,
    ));
    painter.text(
        rect.center(),
        Align2::CENTER_CENTER,
        "Camera off",
        theme::font(11.0),
        Color32::from_rgba_unmultiplied(240, 248, 245, 150),
    );
}

fn call_button(
    ui: &mut Ui,
    color: Color32,
    draw: impl Fn(&egui::Painter, Rect, Color32),
) -> Response {
    let (rect, resp) = ui.allocate_exact_size(Vec2::splat(46.0), Sense::click());
    ui.painter().circle_filled(
        rect.center(),
        23.0,
        if resp.hovered() { darken(color, 14) } else { color },
    );
    draw(ui.painter(), rect.shrink(13.0), Color32::WHITE);
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
}

fn lighten(c: Color32, amount: i32) -> Color32 {
    Color32::from_rgb(
        (c.r() as i32 + amount).min(255) as u8,
        (c.g() as i32 + amount).min(255) as u8,
        (c.b() as i32 + amount).min(255) as u8,
    )
}

fn darken(c: Color32, amount: i32) -> Color32 {
    Color32::from_rgb(
        (c.r() as i32 - amount).max(0) as u8,
        (c.g() as i32 - amount).max(0) as u8,
        (c.b() as i32 - amount).max(0) as u8,
    )
}
