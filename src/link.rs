//! The desktop side of Feather Link: create or join a small message hub and
//! chat with real accounts, including the phone web client the hub serves.

use crate::icons;
use crate::net::{self, ConvView, Hub, HubClient, MsgView, Snapshot};
use crate::theme::{self, Palette};
use eframe::egui::{
    self, Align, Align2, Color32, FontId, Frame, Id, Layout, Margin, Pos2, Rect, RichText,
    Rounding, ScrollArea, Sense, Stroke, TextEdit, Ui, Vec2,
};
use std::time::Duration;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Tab {
    Create,
    Join,
}

pub struct LinkState {
    tab: Tab,
    name: String,
    pin: String,
    code: String,
    port: String,
    client: Option<HubClient>,
    hub: Option<Hub>,
    host_port: Option<u16>,
    firewall_ok: Option<bool>,
    firewall_busy: bool,
    snap: Snapshot,
    selected: Option<u64>,
    draft: String,
    search: String,
    reply: Option<(u64, String, String)>,
    action_msg: Option<u64>,
    show_new: bool,
    picked: Vec<String>,
    group_title: String,
    busy: bool,
    error: Option<String>,
    notice: Option<(String, f64)>,
    last_poll: f64,
    last_typing: f64,
    last_draft: String,
    last_msg_id: u64,
    read_sent: u64,
    scroll_bottom: bool,
    poll_failures: u32,
}

impl LinkState {
    pub fn new() -> Self {
        Self {
            tab: Tab::Create,
            name: String::new(),
            pin: String::new(),
            code: String::new(),
            port: "8777".to_owned(),
            client: None,
            hub: None,
            host_port: None,
            firewall_ok: None,
            firewall_busy: false,
            snap: Snapshot::default(),
            selected: None,
            draft: String::new(),
            search: String::new(),
            reply: None,
            action_msg: None,
            show_new: false,
            picked: Vec::new(),
            group_title: String::new(),
            busy: false,
            error: None,
            notice: None,
            last_poll: 0.0,
            last_typing: 0.0,
            last_draft: String::new(),
            last_msg_id: 0,
            read_sent: 0,
            scroll_bottom: false,
            poll_failures: 0,
        }
    }

    pub fn connected(&self) -> bool {
        self.client.is_some()
    }

    fn toast(&mut self, text: impl Into<String>, now: f64) {
        self.notice = Some((text.into(), now + 3.0));
    }

    pub fn tick(&mut self, ctx: &egui::Context) {
        let now = ctx.input(|i| i.time);
        if self.notice.as_ref().map(|n| now > n.1).unwrap_or(false) {
            self.notice = None;
        }
        if let Some(client) = self.client.clone() {
            let interval = if self.poll_failures == 0 {
                1.15
            } else {
                (1.15 * (1 << self.poll_failures.min(3)) as f64).min(9.0)
            };
            if now - self.last_poll > interval {
                self.last_poll = now;
                match client.snapshot() {
                    Ok(snap) => {
                        self.poll_failures = 0;
                        let previous = self
                            .selected
                            .and_then(|id| self.snap.convs.iter().find(|c| c.id == id))
                            .and_then(|c| c.messages.last())
                            .map(|m| m.id)
                            .unwrap_or(0);
                        self.snap = snap;
                        if self.selected.is_none() {
                            self.selected = self.snap.convs.first().map(|c| c.id);
                            self.scroll_bottom = true;
                        }
                        let current = self
                            .selected
                            .and_then(|id| self.snap.convs.iter().find(|c| c.id == id))
                            .and_then(|c| c.messages.last())
                            .map(|m| m.id)
                            .unwrap_or(0);
                        if current != previous {
                            self.scroll_bottom = true;
                        }
                        self.mark_read(&client);
                    }
                    Err(e) => {
                        if e.contains("sign in") {
                            self.toast("This space asked you to sign in again.", now);
                            self.client = None;
                            self.snap = Snapshot::default();
                            self.selected = None;
                            return;
                        }
                        self.poll_failures += 1;
                        if self.poll_failures == 2 {
                            self.toast(e, now);
                        }
                    }
                }
            }
            ctx.request_repaint_after(Duration::from_millis(700));
        }
    }

    fn mark_read(&mut self, client: &HubClient) {
        let Some(id) = self.selected else { return };
        let Some(conv) = self.snap.convs.iter().find(|c| c.id == id) else {
            return;
        };
        let latest = conv.messages.last().map(|m| m.id).unwrap_or(0);
        if latest > self.read_sent {
            self.read_sent = latest;
            let _ = client.read(conv.id);
        }
    }

    pub fn ui(&mut self, ctx: &egui::Context, pal: &Palette) {
        self.tick(ctx);
        if self.client.is_some() {
            self.connected_ui(ctx, pal);
        } else {
            egui::CentralPanel::default()
                .frame(Frame::none().fill(pal.wallpaper))
                .show(ctx, |ui| self.setup_ui(ui, pal));
        }
        self.toast_ui(ctx, pal);
    }

    // ------------------------------------------------------------- setup
    fn setup_ui(&mut self, ui: &mut Ui, pal: &Palette) {
        let now = ui.input(|i| i.time);
        let width = 548.0;
        ui.add_space(38.0);
        ui.horizontal(|ui| {
            ui.add_space(((ui.available_width() - width) * 0.5).max(16.0));
            ui.vertical(|ui| {
                ui.set_width(width);
                ui.set_max_width(width);
                ui.vertical_centered(|ui| {
                    let (mark, _) =
                        ui.allocate_exact_size(Vec2::splat(46.0), Sense::hover());
                    icons::leaf(ui.painter(), mark, pal.accent, 1.6);
                    ui.add_space(8.0);
                    ui.label(
                        RichText::new("Feather Link")
                            .font(theme::semibold(28.0))
                            .color(pal.text),
                    );
                    ui.label(
                        RichText::new("Real messages, kept on your own little hub.")
                            .font(theme::font(13.0))
                            .color(pal.weak),
                    );
                });
                ui.add_space(20.0);

                Frame::none()
                    .fill(pal.panel)
                    .rounding(Rounding::same(18.0))
                    .stroke(Stroke::new(1.0_f32, pal.border))
                    .inner_margin(Margin::same(24.0))
                    .show(ui, |ui| {
                        ui.set_width(width - 48.0);
                        ui.horizontal(|ui| {
                            if tab_button(ui, pal, "Create a space", self.tab == Tab::Create)
                                .clicked()
                            {
                                self.tab = Tab::Create;
                                self.error = None;
                            }
                            ui.add_space(6.0);
                            if tab_button(ui, pal, "Join a space", self.tab == Tab::Join)
                                .clicked()
                            {
                                self.tab = Tab::Join;
                                self.error = None;
                            }
                        });
                        ui.add_space(16.0);

                        match self.tab {
                            Tab::Create => self.create_form(ui, pal, now),
                            Tab::Join => self.join_form(ui, pal, now),
                        }

                        if let Some(err) = &self.error {
                            ui.add_space(10.0);
                            ui.label(
                                RichText::new(err)
                                    .font(theme::font(12.0))
                                    .color(pal.danger),
                            );
                        }
                    });

                ui.add_space(16.0);
                ui.vertical_centered(|ui| {
                    ui.label(
                        RichText::new("Phones on the same Wi-Fi open the address in a browser and can add it to their home screen. Windows may ask once to let Feather through the firewall.")
                            .font(theme::font(11.5))
                            .color(pal.weak),
                    );
                    ui.add_space(3.0);
                    ui.label(
                        RichText::new("Best on your home Wi-Fi · traffic is not end-to-end encrypted yet")
                            .font(theme::font(10.0))
                            .color(pal.weak),
                    );
                });
            });
        });
    }

    fn create_form(&mut self, ui: &mut Ui, pal: &Palette, now: f64) {
        ui.label(
            RichText::new("This computer becomes the hub. It stays awake while Feather is open.")
                .font(theme::font(12.5))
                .color(pal.weak),
        );
        ui.add_space(14.0);
        self.field(ui, pal, "Your name", "e.g. Sam", false);
        self.field(ui, pal, "PIN (4+ characters)", "keeps your account yours", true);
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            ui.label(
                RichText::new("Port")
                    .font(theme::font(11.5))
                    .color(pal.weak),
            );
            input_box(ui, pal, &mut self.port, "8777", false, 74.0);
            ui.label(
                RichText::new(format!(
                    "share at {}",
                    net::hub_url(self.port.parse().unwrap_or(8777))
                ))
                .font(theme::font(11.0))
                .color(pal.weak),
            );
        });
        ui.add_space(14.0);
        let label = if self.busy { "Starting…" } else { "Create my space" };
        if primary_button(ui, pal, label).clicked() && !self.busy {
            self.create_space(now);
        }
    }

    fn join_form(&mut self, ui: &mut Ui, pal: &Palette, now: f64) {
        ui.label(
            RichText::new("Ask the person hosting the space for its address, then sign in with your own name.")
                .font(theme::font(12.5))
                .color(pal.weak),
        );
        ui.add_space(14.0);
        ui.label(
            RichText::new("Space address")
                .font(theme::font(11.5))
                .color(pal.weak),
        );
        input_box(
            ui,
            pal,
            &mut self.code,
            "192.168.1.5:8777 or https://chat.example.com",
            false,
            f32::INFINITY,
        );
        ui.add_space(4.0);
        ui.label(
            RichText::new("A home hub uses its Wi-Fi address. A hub on a server uses a name starting with https://.")
                .font(theme::font(10.5))
                .color(pal.weak),
        );
        ui.add_space(10.0);
        self.field(ui, pal, "Your name", "e.g. Sam", false);
        self.field(ui, pal, "PIN (4+ characters)", "keeps your account yours", true);
        ui.add_space(6.0);
        let label = if self.busy { "Connecting…" } else { "Join this space" };
        if primary_button(ui, pal, label).clicked() && !self.busy {
            self.join_space(now);
        }
    }

    fn field(&mut self, ui: &mut Ui, pal: &Palette, label: &str, hint: &str, password: bool) {
        ui.label(
            RichText::new(label)
                .font(theme::font(11.5))
                .color(pal.weak),
        );
        if password {
            input_box(ui, pal, &mut self.pin, hint, true, f32::INFINITY);
        } else {
            input_box(ui, pal, &mut self.name, hint, false, f32::INFINITY);
        }
        ui.add_space(10.0);
    }

    fn create_space(&mut self, now: f64) {
        self.busy = true;
        self.error = None;
        let result = (|| -> Result<(Hub, HubClient), String> {
            validate(&self.name, &self.pin)?;
            let port: u16 = self
                .port
                .trim()
                .parse()
                .map_err(|_| "The port should be a number like 8777.".to_owned())?;
            let hub = Hub::start(port, net::default_data_path())?;
            let client = HubClient::connect(
                &format!("127.0.0.1:{}", hub.port),
                self.name.trim(),
                self.pin.trim(),
            )?;
            Ok((hub, client))
        })();
        self.busy = false;
        match result {
            Ok((hub, client)) => {
                let port = hub.port;
                self.host_port = Some(port);
                self.hub = Some(hub);
                self.firewall_ok = Some(net::firewall_rule_exists(port));
                self.client = Some(client);
                self.enter(now);
            }
            Err(e) => self.error = Some(e),
        }
    }

    fn join_space(&mut self, now: f64) {
        self.busy = true;
        self.error = None;
        let result = (|| -> Result<HubClient, String> {
            validate(&self.name, &self.pin)?;
            let code = net::normalize_code(&self.code);
            if code.is_empty() {
                return Err("Enter the space address first.".to_owned());
            }
            HubClient::connect(&code, self.name.trim(), self.pin.trim())
        })();
        self.busy = false;
        match result {
            Ok(client) => {
                self.hub = None;
                self.host_port = None;
                self.firewall_ok = None;
                self.firewall_busy = false;
                self.client = Some(client);
                self.enter(now);
            }
            Err(e) => self.error = Some(e),
        }
    }

    /// Used only by the screenshot preview to open a known hub.
    pub fn preview_join(&mut self, code: &str, name: &str, pin: &str) {
        self.name = name.to_owned();
        self.pin = pin.to_owned();
        if let Ok(client) = HubClient::connect(code, name, pin) {
            self.client = Some(client);
            if let Ok(snap) = self.client.as_ref().unwrap().snapshot() {
                self.snap = snap;
                self.selected = self.snap.convs.first().map(|c| c.id);
                self.scroll_bottom = true;
            }
        }
    }

    /// Reopens the last space on launch, so the app returns to real chats.
    pub fn resume(&mut self) -> bool {
        let Some(saved) = net::load_saved_link() else {
            return false;
        };
        // If a hub is already running on the saved host port (a second app
        // window or `--serve`), join it instead of starting a duplicate.
        if let Some(port) = saved.host_port {
            let probe = HubClient {
                base: format!("127.0.0.1:{port}"),
                token: saved.token.clone(),
                me: saved.me.clone(),
            };
            if let Ok(snap) = probe.snapshot() {
                self.hub = None;
                self.host_port = Some(port);
                self.client = Some(probe);
                self.snap = snap;
                self.selected = self.snap.convs.first().map(|c| c.id);
                self.scroll_bottom = true;
                self.read_sent = 0;
                self.persist();
                return true;
            }
        }

        let mut hub = None;
        let base = if let Some(port) = saved.host_port {
            match Hub::start(port, net::default_data_path()) {
                Ok(h) => {
                    let base = format!("127.0.0.1:{}", h.port);
                    hub = Some(h);
                    base
                }
                Err(_) => {
                    self.code = saved.base.clone();
                    self.name = saved.me.name.clone();
                    return false;
                }
            }
        } else {
            saved.base.clone()
        };
        let client = HubClient {
            base: base.clone(),
            token: saved.token.clone(),
            me: saved.me.clone(),
        };
        match client.snapshot() {
            Ok(snap) => {
                self.host_port = hub.as_ref().map(|h| h.port);
                self.firewall_ok = hub.as_ref().map(|h| net::firewall_rule_exists(h.port));
                self.hub = hub;
                self.client = Some(client);
                self.snap = snap;
                self.selected = self.snap.convs.first().map(|c| c.id);
                self.scroll_bottom = true;
                self.read_sent = 0;
                self.persist();
                true
            }
            Err(_) => {
                self.hub = None;
                self.code = saved.base.clone();
                self.name = saved.me.name.clone();
                false
            }
        }
    }

    fn persist(&self) {
        if let Some(client) = &self.client {
            net::save_saved_link(&net::SavedLink {
                base: client.base.clone(),
                token: client.token.clone(),
                me: client.me.clone(),
                host_port: self.host_port,
            });
        }
    }

    fn enter(&mut self, now: f64) {
        self.snap = Snapshot::default();
        self.selected = None;
        self.last_msg_id = 0;
        self.read_sent = 0;
        self.error = None;
        self.toast("Connected. Say hello.", now);
        if let Some(client) = self.client.clone() {
            match client.snapshot() {
                Ok(snap) => {
                    self.snap = snap;
                    self.selected = self.snap.convs.first().map(|c| c.id);
                    self.scroll_bottom = true;
                    self.mark_read(&client);
                    self.persist();
                }
                Err(e) => self.error = Some(e),
            }
        }
    }

    fn disconnect(&mut self) {
        self.client = None;
        self.hub = None;
        self.host_port = None;
        self.firewall_ok = None;
        self.firewall_busy = false;
        self.snap = Snapshot::default();
        self.selected = None;
        self.reply = None;
        self.action_msg = None;
        self.draft.clear();
        net::clear_saved_link();
    }

    // --------------------------------------------------------- connected
    fn connected_ui(&mut self, ctx: &egui::Context, pal: &Palette) {
        egui::SidePanel::left("link_side")
            .exact_width(318.0)
            .resizable(false)
            .frame(Frame::none().fill(pal.panel))
            .show(ctx, |ui| self.side_ui(ui, pal));
        egui::CentralPanel::default()
            .frame(Frame::none().fill(pal.wallpaper))
            .show(ctx, |ui| self.thread_ui(ui, pal));

        if self.show_new {
            self.new_chat_window(ctx, pal);
        }
    }

    fn side_ui(&mut self, ui: &mut Ui, pal: &Palette) {
        let rect = ui.max_rect();
        ui.painter().line_segment(
            [
                Pos2::new(rect.right() - 0.5, rect.top()),
                Pos2::new(rect.right() - 0.5, rect.bottom()),
            ],
            Stroke::new(1.0, pal.border),
        );
        ui.add_space(16.0);
        ui.horizontal(|ui| {
            ui.add_space(16.0);
            let (mark, _) = ui.allocate_exact_size(Vec2::splat(24.0), Sense::hover());
            icons::leaf(ui.painter(), mark, pal.accent, 1.4);
            ui.add_space(6.0);
            ui.vertical(|ui| {
                ui.label(
                    RichText::new("Feather Link")
                        .font(theme::semibold(17.0))
                        .color(pal.text),
                );
                ui.label(
                    RichText::new("live · connected")
                        .font(theme::font(9.5))
                        .color(pal.accent),
                );
            });
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.add_space(10.0);
                let (dot, _) = ui.allocate_exact_size(Vec2::splat(9.0), Sense::hover());
                ui.painter().circle_filled(dot.center(), 4.0, pal.accent);
            });
        });

        ui.add_space(12.0);
        if let Some(port) = self.hub.as_ref().map(|h| h.port) {
            let url = net::hub_url(port);
            Frame::none()
                .fill(pal.accent_soft)
                .rounding(Rounding::same(10.0))
                .inner_margin(Margin::symmetric(10.0, 8.0))
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.horizontal(|ui| {
                        ui.vertical(|ui| {
                            ui.label(
                                RichText::new("Phones join at")
                                    .font(theme::font(9.5))
                                    .color(pal.weak),
                            );
                            ui.label(
                                RichText::new(&url)
                                    .font(theme::semibold(11.5))
                                    .color(pal.accent_deep),
                            );
                        });
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if small_button(ui, pal, "Copy").clicked() {
                                ui.output_mut(|o| o.copied_text = url.clone());
                            }
                        });
                    });
                });
            ui.add_space(6.0);
            self.firewall_row(ui, pal, port);
            ui.add_space(8.0);
        } else if let Some(client) = &self.client {
            Frame::none()
                .fill(pal.panel_alt)
                .rounding(Rounding::same(10.0))
                .inner_margin(Margin::symmetric(10.0, 7.0))
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.label(
                        RichText::new(format!("Joined {}", client.base))
                            .font(theme::font(10.0))
                            .color(pal.weak),
                    );
                });
            ui.add_space(8.0);
        }
        if self.poll_failures > 0 {
            ui.horizontal(|ui| {
                ui.add_space(16.0);
                ui.label(
                    RichText::new("Reconnecting…")
                        .font(theme::font(10.5))
                        .color(pal.danger),
                );
            });
            ui.add_space(4.0);
        }

        ui.horizontal(|ui| {
            ui.add_space(14.0);
            let field_w = (ui.available_width() - 34.0).max(120.0);
            let mut edit = TextEdit::singleline(&mut self.search)
                .hint_text("Search chats")
                .desired_width(field_w - 20.0)
                .font(theme::font(12.5));
            edit = edit.margin(Vec2::new(10.0, 7.0));
            Frame::none()
                .fill(pal.search_bg)
                .rounding(Rounding::same(9.0))
                .show(ui, |ui| {
                    ui.add(edit);
                });
        });
        ui.add_space(8.0);

        let footer_h = 92.0;
        let list_h = (ui.available_height() - footer_h).max(120.0);
        let query = self.search.trim().to_lowercase();
        let convs: Vec<ConvView> = self
            .snap
            .convs
            .iter()
            .filter(|c| {
                if query.is_empty() {
                    return true;
                }
                c.title.to_lowercase().contains(&query)
                    || c.messages
                        .last()
                        .map(|m| m.text.to_lowercase().contains(&query))
                        .unwrap_or(false)
            })
            .cloned()
            .collect();
        let mut open: Option<u64> = None;
        ScrollArea::vertical()
            .id_source("link_convs")
            .max_height(list_h)
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                if convs.is_empty() {
                    ui.add_space(28.0);
                    ui.vertical_centered(|ui| {
                        ui.label(
                            RichText::new(if query.is_empty() {
                                "No chats yet"
                            } else {
                                "No chats found"
                            })
                            .font(theme::semibold(14.0))
                            .color(pal.text),
                        );
                        ui.add_space(4.0);
                        ui.label(
                            RichText::new(if query.is_empty() {
                                "Start one with the button below."
                            } else {
                                "Try another name or word."
                            })
                            .font(theme::font(11.5))
                            .color(pal.weak),
                        );
                    });
                }
                for conv in &convs {
                    let selected = self.selected == Some(conv.id);
                    if conv_row(ui, pal, conv, selected, self.me_id()).clicked() {
                        open = Some(conv.id);
                    }
                }
            });

        if let Some(id) = open {
            self.selected = Some(id);
            self.read_sent = 0;
            self.scroll_bottom = true;
            self.action_msg = None;
            self.reply = None;
            if let Some(client) = self.client.clone() {
                self.mark_read(&client);
            }
        }

        ui.with_layout(Layout::bottom_up(Align::Min), |ui| {
            ui.add_space(14.0);
            ui.horizontal(|ui| {
                ui.add_space(14.0);
                ui.label(
                    RichText::new(format!("Signed in as {}", self.me_name()))
                        .font(theme::font(10.5))
                        .color(pal.weak),
                );
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.add_space(14.0);
                    if small_button(ui, pal, "Disconnect").clicked() {
                        self.disconnect();
                    }
                });
            });
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                ui.add_space(14.0);
                if accent_pill(ui, pal, "+  New chat").clicked() {
                    self.picked.clear();
                    self.group_title.clear();
                    self.show_new = true;
                }
            });
            ui.add_space(10.0);
            ui.painter().line_segment(
                [
                    Pos2::new(ui.max_rect().left() + 14.0, ui.cursor().top()),
                    Pos2::new(ui.max_rect().right() - 14.0, ui.cursor().top()),
                ],
                Stroke::new(1.0_f32, pal.border),
            );
        });
    }

    /// One small row under the share address: Windows Firewall is the usual
    /// reason a phone on the same Wi-Fi sees nothing, so make the fix a click.
    fn firewall_row(&mut self, ui: &mut Ui, pal: &Palette, port: u16) {
        if self.firewall_ok.is_none() {
            self.firewall_ok = Some(net::firewall_rule_exists(port));
        }
        let ready = self.firewall_ok.unwrap_or(false);
        ui.horizontal(|ui| {
            ui.add_space(16.0);
            if ready {
                let (dot, _) = ui.allocate_exact_size(Vec2::splat(8.0), Sense::hover());
                ui.painter().circle_filled(dot.center(), 3.2, pal.accent);
                ui.label(
                    RichText::new("Firewall open for phones")
                        .font(theme::font(9.8))
                        .color(pal.weak),
                );
            } else {
                ui.label(
                    RichText::new("Phone can't connect?")
                        .font(theme::font(9.8))
                        .color(pal.weak),
                );
                let label = if self.firewall_busy {
                    "Allowing…"
                } else {
                    "Allow firewall"
                };
                let response = small_button(ui, pal, label).on_hover_text(
                    "Opens only this chat port in Windows Firewall. Windows asks you to approve it.",
                );
                if response.clicked() && !self.firewall_busy {
                    self.firewall_busy = true;
                    let result = net::allow_firewall(port);
                    self.firewall_busy = false;
                    let now = ui.input(|i| i.time);
                    match result {
                        Ok(()) => {
                            self.firewall_ok = Some(net::firewall_rule_exists(port));
                            if self.firewall_ok == Some(true) {
                                self.toast(
                                    "Firewall opened. A phone on this Wi-Fi can join now.",
                                    now,
                                );
                            } else {
                                self.toast(
                                    "Windows did not add the rule. Allow Feather if Windows asks.",
                                    now,
                                );
                            }
                        }
                        Err(e) => self.toast(e, now),
                    }
                }
            }
        });
    }

    fn me_id(&self) -> &str {
        self.client.as_ref().map(|c| c.me.id.as_str()).unwrap_or("")
    }

    fn me_name(&self) -> String {
        self.client
            .as_ref()
            .map(|c| c.me.name.clone())
            .unwrap_or_default()
    }

    fn thread_ui(&mut self, ui: &mut Ui, pal: &Palette) {
        let Some(conv) = self
            .selected
            .and_then(|id| self.snap.convs.iter().find(|c| c.id == id).cloned())
        else {
            ui.vertical_centered(|ui| {
                ui.add_space(120.0);
                let (mark, _) = ui.allocate_exact_size(Vec2::splat(54.0), Sense::hover());
                icons::leaf(ui.painter(), mark, pal.accent, 1.6);
                ui.add_space(10.0);
                ui.label(
                    RichText::new("Pick a chat")
                        .font(theme::semibold(20.0))
                        .color(pal.text),
                );
                ui.label(
                    RichText::new("or start a new one with the button on the left.")
                        .font(theme::font(12.5))
                        .color(pal.weak),
                );
            });
            return;
        };

        egui::TopBottomPanel::top("link_thread_head")
            .exact_height(72.0)
            .frame(
                Frame::none()
                    .fill(pal.header)
                    .inner_margin(Margin::symmetric(0.0, 6.0)),
            )
            .show_inside(ui, |ui| {
                let rect = ui.max_rect();
                ui.painter().line_segment(
                    [
                        Pos2::new(rect.left(), rect.bottom() - 0.5),
                        Pos2::new(rect.right(), rect.bottom() - 0.5),
                    ],
                    Stroke::new(1.0, pal.border),
                );
                let head = ui.max_rect();
                let av = Rect::from_min_size(
                    Pos2::new(head.left() + 18.0, head.center().y - 20.0),
                    Vec2::splat(40.0),
                );
                paint_avatar(
                    ui,
                    av,
                    &conv.title,
                    color_of(if conv.group {
                        0x18a06e
                    } else {
                        member_color(&conv, self.me_id())
                    }),
                    false,
                );
                ui.painter().text(
                    Pos2::new(av.right() + 10.0, head.center().y - 14.0),
                    Align2::LEFT_TOP,
                    &conv.title,
                    theme::semibold(15.0),
                    pal.text,
                );
                let sub = if !conv.typing.is_empty() {
                    if conv.typing.len() == 1 {
                        format!("{} is typing…", conv.typing[0])
                    } else {
                        "Several people are typing…".to_owned()
                    }
                } else if conv.group {
                    format!("{} members", conv.members.len())
                } else if conv
                    .members
                    .iter()
                    .any(|m| m.id != self.me_id() && m.online)
                {
                    "online".to_owned()
                } else {
                    "in this space".to_owned()
                };
                ui.painter().text(
                    Pos2::new(av.right() + 10.0, head.center().y + 2.0),
                    Align2::LEFT_TOP,
                    sub,
                    theme::font(11.0),
                    if conv.typing.is_empty() {
                        pal.weak
                    } else {
                        pal.accent
                    },
                );
            });

        egui::TopBottomPanel::bottom("link_composer")
            .frame(Frame::none().fill(pal.panel))
            .show_inside(ui, |ui| {
                self.composer_ui(ui, pal, &conv);
            });

        egui::CentralPanel::default()
            .frame(Frame::none().fill(pal.wallpaper))
            .show_inside(ui, |ui| {
                paint_wall(ui, pal);
                ScrollArea::vertical()
                    .id_source("link_thread_scroll")
                    .auto_shrink([false, false])
                    .stick_to_bottom(self.scroll_bottom)
                    .show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.add_space(12.0);
                        let mut last_day = String::new();
                        let mut last_sender = String::new();
                        let mut clicked: Option<u64> = None;
                        for msg in &conv.messages {
                            let day = day_label(msg.at);
                            if day != last_day {
                                day_pill(ui, pal, &day);
                                last_day = day;
                                last_sender.clear();
                            }
                            let mine = msg.from == self.me_id();
                            let show_sender = conv.group && !mine && last_sender != msg.from;
                            last_sender = msg.from.clone();
                            if let Some(id) = message_bubble(ui, pal, msg, mine, show_sender) {
                                clicked = Some(id);
                            }
                        }
                        if !conv.typing.is_empty() {
                            typing_bubble(ui, pal, &conv.typing);
                        }
                        ui.add_space(10.0);
                        if let Some(id) = clicked {
                            self.action_msg = Some(id);
                            self.reply = None;
                        }
                    });
                self.scroll_bottom = false;
            });
    }

    fn composer_ui(&mut self, ui: &mut Ui, pal: &Palette, conv: &ConvView) {
        let now = ui.input(|i| i.time);
        ui.add_space(6.0);
        if let Some(id) = self.action_msg {
            if let Some(msg) = conv.messages.iter().find(|m| m.id == id).cloned() {
                Frame::none()
                    .fill(pal.panel_alt)
                    .inner_margin(Margin::symmetric(14.0, 7.0))
                    .show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.horizontal(|ui| {
                            ui.label(
                                RichText::new("Reply").font(theme::semibold(11.0)).color(pal.accent),
                            );
                            if ui
                                .add(egui::Button::new(RichText::new("↩").font(theme::font(15.0))).frame(false))
                                .clicked()
                            {
                                self.reply = Some((msg.id, msg.from_name.clone(), msg.text.clone()));
                                self.action_msg = None;
                            }
                            ui.add_space(8.0);
                            for emoji in ["👍", "❤️", "😂", "😮", "🙏"] {
                                if ui
                                    .add(
                                        egui::Button::new(RichText::new(emoji).font(theme::font(15.0)))
                                            .frame(false),
                                    )
                                    .clicked()
                                {
                                    if let Some(client) = self.client.clone() {
                                        if client.react(msg.id, emoji).is_ok() {
                                            self.action_msg = None;
                                            self.last_poll = 0.0;
                                        }
                                    }
                                }
                            }
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                if ui
                                    .add(
                                        egui::Button::new(
                                            RichText::new("✕").font(theme::font(13.0)).color(pal.weak),
                                        )
                                        .frame(false),
                                    )
                                    .clicked()
                                {
                                    self.action_msg = None;
                                }
                            });
                        });
                    });
            } else {
                self.action_msg = None;
            }
        }

        if let Some((_, name, text)) = self.reply.clone() {
            Frame::none()
                .fill(pal.accent_soft)
                .inner_margin(Margin::symmetric(14.0, 6.0))
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.horizontal(|ui| {
                        ui.vertical(|ui| {
                            ui.label(
                                RichText::new(format!("Replying to {name}"))
                                    .font(theme::semibold(10.5))
                                    .color(pal.accent_deep),
                            );
                            ui.label(
                                RichText::new(&text)
                                    .font(theme::font(11.0))
                                    .color(pal.weak),
                            );
                        });
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if ui
                                .add(
                                    egui::Button::new(
                                        RichText::new("✕").font(theme::font(12.0)).color(pal.weak),
                                    )
                                    .frame(false),
                                )
                                .clicked()
                            {
                                self.reply = None;
                            }
                        });
                    });
                });
        }

        ui.add_space(6.0);
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 6.0;
            ui.add_space(12.0);
            let (mic, _) = ui.allocate_exact_size(Vec2::splat(30.0), Sense::hover());
            icons::smiley(ui.painter(), mic.shrink(4.0), pal.weak, 1.3);
            ui.add_space(2.0);

            let field_w = (ui.available_width() - 150.0).max(120.0);
            let edit = TextEdit::singleline(&mut self.draft)
                .hint_text("Message")
                .desired_width(field_w)
                .font(theme::font(14.0))
                .frame(false);
            let resp = Frame::none()
                .fill(pal.search_bg)
                .rounding(Rounding::same(19.0))
                .inner_margin(Margin::symmetric(14.0, 9.0))
                .show(ui, |ui| {
                    ui.set_width(field_w);
                    ui.add(edit)
                })
                .inner;

            if resp.changed() && !self.draft.trim().is_empty() {
                if now - self.last_typing > 1.4 {
                    self.last_typing = now;
                    if let Some(client) = self.client.clone() {
                        let _ = client.typing(conv.id);
                    }
                }
            }

            let enter = resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
            ui.add_space(6.0);
            let (send_rect, send_resp) =
                ui.allocate_exact_size(Vec2::splat(42.0), Sense::click());
            let hot = send_resp.hovered();
            ui.painter().circle_filled(
                send_rect.center(),
                21.0,
                if hot { pal.accent_deep } else { pal.accent },
            );
            icons::send(
                ui.painter(),
                send_rect.shrink(12.0),
                Color32::WHITE,
                1.7,
            );
            if (send_resp.clicked() || enter) && !self.draft.trim().is_empty() {
                self.send(conv.id, now);
            }
            ui.add_space(10.0);
        });
        ui.add_space(8.0);
    }

    fn send(&mut self, conv: u64, now: f64) {
        let text = self.draft.trim().to_owned();
        if text.is_empty() {
            return;
        }
        let reply = self.reply.as_ref().map(|r| r.0);
        if let Some(client) = self.client.clone() {
            match client.send(conv, &text, reply) {
                Ok(()) => {
                    self.draft.clear();
                    self.reply = None;
                    self.action_msg = None;
                    self.last_poll = 0.0;
                    self.scroll_bottom = true;
                    if let Ok(snap) = client.snapshot() {
                        self.snap = snap;
                        self.mark_read(&client);
                    }
                }
                Err(e) => self.toast(e, now),
            }
        }
    }

    fn new_chat_window(&mut self, ctx: &egui::Context, pal: &Palette) {
        let people: Vec<_> = self
            .snap
            .people
            .iter()
            .filter(|p| p.id != self.me_id())
            .cloned()
            .collect();
        let mut open = self.show_new;
        egui::Window::new("New chat")
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
            .frame(
                Frame::window(&ctx.style())
                    .fill(pal.panel)
                    .stroke(Stroke::new(1.0, pal.border))
                    .rounding(Rounding::same(14.0))
                    .inner_margin(Margin::same(18.0)),
            )
            .show(ctx, |ui| {
                ui.set_width(340.0);
                ui.label(
                    RichText::new("Pick one person, or several for a group.")
                        .font(theme::font(11.5))
                        .color(pal.weak),
                );
                ui.add_space(10.0);
                if people.is_empty() {
                    ui.label(
                        RichText::new("No one else has joined this space yet. Share the address from the left.")
                            .font(theme::font(12.0))
                            .color(pal.weak),
                    );
                }
                ScrollArea::vertical()
                    .id_source("link_people")
                    .max_height(300.0)
                    .auto_shrink([false, true])
                    .show(ui, |ui| {
                        for person in &people {
                            let picked = self.picked.contains(&person.id);
                            let label = format!(
                                "{}  {}",
                                if picked { "✓" } else { "○" },
                                person.name
                            );
                            if ui
                                .add(
                                    egui::Button::new(
                                        RichText::new(label).font(theme::font(13.5)).color(
                                            if picked { pal.accent_deep } else { pal.text },
                                        ),
                                    )
                                    .fill(if picked { pal.accent_soft } else { pal.panel_alt })
                                    .rounding(Rounding::same(9.0))
                                    .min_size(Vec2::new(ui.available_width(), 34.0)),
                                )
                                .clicked()
                            {
                                if picked {
                                    self.picked.retain(|id| id != &person.id);
                                } else {
                                    self.picked.push(person.id.clone());
                                }
                            }
                            ui.add_space(4.0);
                        }
                    });
                if self.picked.len() > 1 {
                    ui.add_space(6.0);
                    ui.label(
                        RichText::new("Group name (optional)")
                            .font(theme::font(10.5))
                            .color(pal.weak),
                    );
                    ui.add(
                        TextEdit::singleline(&mut self.group_title)
                            .hint_text("Weekend plans")
                            .desired_width(f32::INFINITY)
                            .font(theme::font(13.0)),
                    );
                }
                ui.add_space(12.0);
                ui.horizontal(|ui| {
                    let ready = !self.picked.is_empty() && !self.busy;
                    let label = if self.picked.len() > 1 {
                        format!("Start group ({})", self.picked.len())
                    } else {
                        "Start chat".to_owned()
                    };
                    let btn = ui.add_enabled(
                        ready,
                        egui::Button::new(RichText::new(label).color(Color32::WHITE).font(theme::semibold(13.0)))
                            .fill(pal.accent)
                            .rounding(Rounding::same(10.0))
                            .min_size(Vec2::new(140.0, 38.0)),
                    );
                    if btn.clicked() {
                        if let Some(client) = self.client.clone() {
                            match client.create_conv(&self.picked, &self.group_title) {
                                Ok(id) => {
                                    self.show_new = false;
                                    self.picked.clear();
                                    self.group_title.clear();
                                    self.selected = Some(id);
                                    self.read_sent = 0;
                                    self.scroll_bottom = true;
                                    self.last_poll = 0.0;
                                    if let Ok(snap) = client.snapshot() {
                                        self.snap = snap;
                                    }
                                }
                                Err(e) => {
                                    self.toast(e, ctx.input(|i| i.time));
                                }
                            }
                        }
                    }
                });
            });
        self.show_new = open;
    }

    fn toast_ui(&mut self, ctx: &egui::Context, pal: &Palette) {
        let now = ctx.input(|i| i.time);
        let Some((text, until)) = self.notice.clone() else {
            return;
        };
        if now > until {
            self.notice = None;
            return;
        }
        egui::Area::new(Id::new("link_toast"))
            .anchor(Align2::CENTER_BOTTOM, Vec2::new(0.0, -28.0))
            .show(ctx, |ui| {
                Frame::none()
                    .fill(pal.text)
                    .rounding(Rounding::same(10.0))
                    .inner_margin(Margin::symmetric(14.0, 8.0))
                    .show(ui, |ui| {
                        ui.label(
                            RichText::new(text)
                                .font(theme::font(12.0))
                                .color(pal.panel),
                        );
                    });
            });
        ctx.request_repaint_after(Duration::from_millis(200));
    }
}

// ------------------------------------------------------------- small pieces

fn validate(name: &str, pin: &str) -> Result<(), String> {
    if name.trim().len() < 2 {
        return Err("Pick a name with at least 2 letters.".to_owned());
    }
    if pin.trim().len() < 4 {
        return Err("Your PIN needs at least 4 characters.".to_owned());
    }
    Ok(())
}

fn color_of(value: u32) -> Color32 {
    Color32::from_rgb(
        ((value >> 16) & 0xff) as u8,
        ((value >> 8) & 0xff) as u8,
        (value & 0xff) as u8,
    )
}

fn member_color(conv: &ConvView, me: &str) -> u32 {
    conv.members
        .iter()
        .find(|m| m.id != me)
        .or_else(|| conv.members.first())
        .map(|m| m.color)
        .unwrap_or(0x347868)
}

fn initials(name: &str) -> String {
    let mut out = String::new();
    for word in name.split_whitespace().take(2) {
        if let Some(ch) = word.chars().next() {
            out.push(ch.to_ascii_uppercase());
        }
    }
    if out.is_empty() {
        "?".to_owned()
    } else {
        out
    }
}

fn paint_avatar(ui: &Ui, rect: Rect, name: &str, color: Color32, online: bool) {
    ui.painter().circle_filled(rect.center(), rect.width() * 0.5, color);
    ui.painter().text(
        rect.center(),
        Align2::CENTER_CENTER,
        initials(name),
        theme::semibold(13.0),
        Color32::WHITE,
    );
    if online {
        let c = rect.right_bottom() + Vec2::new(-4.0, -4.0);
        ui.painter().circle_filled(c, 5.0, Color32::WHITE);
        ui.painter().circle_filled(c, 3.6, Color32::from_rgb(64, 196, 140));
    }
}

fn input_box(
    ui: &mut Ui,
    pal: &Palette,
    text: &mut String,
    hint: &str,
    password: bool,
    width: f32,
) -> egui::Response {
    Frame::none()
        .fill(pal.panel_alt)
        .rounding(Rounding::same(9.0))
        .stroke(Stroke::new(1.0_f32, pal.border))
        .inner_margin(Margin::symmetric(12.0, 8.0))
        .show(ui, |ui| {
            if width.is_finite() {
                ui.set_width(width);
            } else {
                ui.set_width(ui.available_width());
            }
            let mut edit = TextEdit::singleline(text)
                .hint_text(hint)
                .font(theme::font(14.0))
                .frame(false)
                .desired_width(if width.is_finite() { width } else { f32::INFINITY });
            if password {
                edit = edit.password(true);
            }
            ui.add(edit)
        })
        .inner
}

fn tab_button(ui: &mut Ui, pal: &Palette, label: &str, active: bool) -> egui::Response {
    ui.add(
        egui::Button::new(
            RichText::new(label)
                .font(theme::semibold(13.0))
                .color(if active { pal.text } else { pal.weak }),
        )
        .fill(if active { pal.panel_alt } else { Color32::TRANSPARENT })
        .rounding(Rounding::same(9.0))
        .min_size(Vec2::new(140.0, 34.0)),
    )
}

fn primary_button(ui: &mut Ui, pal: &Palette, label: &str) -> egui::Response {
    let (rect, resp) =
        ui.allocate_exact_size(Vec2::new(ui.available_width(), 42.0), Sense::click());
    ui.painter().rect_filled(
        rect,
        Rounding::same(11.0),
        if resp.hovered() { pal.accent_deep } else { pal.accent },
    );
    ui.painter().text(
        rect.center(),
        Align2::CENTER_CENTER,
        label,
        theme::semibold(14.0),
        Color32::WHITE,
    );
    resp
}

fn small_button(ui: &mut Ui, pal: &Palette, label: &str) -> egui::Response {
    ui.add(
        egui::Button::new(RichText::new(label).font(theme::font(10.5)).color(pal.weak))
            .fill(pal.panel)
            .stroke(Stroke::new(1.0, pal.border))
            .rounding(Rounding::same(7.0)),
    )
}

fn accent_pill(ui: &mut Ui, pal: &Palette, label: &str) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(
        Vec2::new((ui.available_width() - 14.0).max(100.0), 34.0),
        Sense::click(),
    );
    ui.painter().rect_filled(
        rect,
        Rounding::same(9.0),
        if resp.hovered() { pal.accent_deep } else { pal.accent },
    );
    ui.painter().text(
        rect.center(),
        Align2::CENTER_CENTER,
        label,
        theme::semibold(12.0),
        Color32::WHITE,
    );
    resp
}

fn conv_row(
    ui: &mut Ui,
    pal: &Palette,
    conv: &ConvView,
    selected: bool,
    me: &str,
) -> egui::Response {
    let h = 62.0;
    let (rect, resp) = ui.allocate_exact_size(
        Vec2::new(ui.available_width(), h),
        Sense::click(),
    );
    if selected {
        ui.painter().rect_filled(rect, Rounding::same(0.0), pal.selected);
    } else if resp.hovered() {
        ui.painter().rect_filled(rect, Rounding::same(0.0), pal.hover);
    }
    let av = Rect::from_center_size(
        Pos2::new(rect.left() + 34.0, rect.center().y),
        Vec2::splat(40.0),
    );
    let other = conv.members.iter().find(|m| m.id != me);
    paint_avatar(
        ui,
        av,
        &conv.title,
        color_of(if conv.group {
            0x18a06e
        } else {
            other.map(|m| m.color).unwrap_or(0x347868)
        }),
        other.map(|m| m.online).unwrap_or(false),
    );

    let last = conv.messages.last();
    let preview = if !conv.typing.is_empty() {
        "typing…".to_owned()
    } else if let Some(m) = last {
        let mine = m.from == me;
        let prefix = if mine {
            "You: ".to_owned()
        } else if conv.group {
            format!("{}: ", m.from_name)
        } else {
            String::new()
        };
        format!("{}{}", prefix, m.text.replace('\n', " "))
    } else {
        "Say hello".to_owned()
    };

    let text_x = rect.left() + 62.0;
    ui.painter().text(
        Pos2::new(text_x, rect.top() + 12.0),
        Align2::LEFT_TOP,
        ellipsize(ui, &conv.title, theme::semibold(13.5), rect.width() - 130.0),
        theme::semibold(13.5),
        pal.text,
    );
    let time = last.map(|m| short_time(m.at)).unwrap_or_default();
    ui.painter().text(
        Pos2::new(rect.right() - 14.0, rect.top() + 13.0),
        Align2::RIGHT_TOP,
        time,
        theme::font(9.5),
        if conv.unread > 0 { pal.accent } else { pal.weak },
    );
    ui.painter().text(
        Pos2::new(text_x, rect.top() + 34.0),
        Align2::LEFT_TOP,
        ellipsize(
            ui,
            &preview,
            theme::font(11.5),
            rect.width() - 92.0,
        ),
        theme::font(11.5),
        if !conv.typing.is_empty() { pal.accent } else { pal.weak },
    );
    if conv.unread > 0 {
        let badge = Rect::from_center_size(
            Pos2::new(rect.right() - 20.0, rect.bottom() - 17.0),
            Vec2::new(19.0, 18.0),
        );
        ui.painter().rect_filled(badge, Rounding::same(9.0), pal.badge);
        ui.painter().text(
            badge.center(),
            Align2::CENTER_CENTER,
            if conv.unread > 9 { "9+".to_owned() } else { conv.unread.to_string() },
            theme::semibold(9.5),
            Color32::WHITE,
        );
    }
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
}

fn ellipsize(ui: &Ui, text: &str, font: FontId, max_w: f32) -> String {
    if max_w <= 8.0 {
        return String::new();
    }
    let galley =
        ui.fonts(|f| f.layout_no_wrap(text.to_owned(), font.clone(), Color32::BLACK));
    if galley.size().x <= max_w {
        return text.to_owned();
    }
    let mut cut = text.to_owned();
    while !cut.is_empty() {
        cut.pop();
        let g = ui.fonts(|f| {
            f.layout_no_wrap(format!("{cut}…"), font.clone(), Color32::BLACK)
        });
        if g.size().x <= max_w {
            return format!("{cut}…");
        }
    }
    "…".to_owned()
}

fn short_time(at: i64) -> String {
    use chrono::TimeZone;
    let Some(dt) = chrono::Local.timestamp_opt(at, 0).single() else {
        return String::new();
    };
    let now = chrono::Local::now();
    if dt.date_naive() == now.date_naive() {
        dt.format("%H:%M").to_string()
    } else if dt.date_naive() == now.date_naive() - chrono::Duration::days(1) {
        "Yesterday".to_owned()
    } else {
        dt.format("%d %b").to_string()
    }
}

fn day_label(at: i64) -> String {
    use chrono::TimeZone;
    let Some(dt) = chrono::Local.timestamp_opt(at, 0).single() else {
        return String::new();
    };
    let now = chrono::Local::now();
    if dt.date_naive() == now.date_naive() {
        "Today".to_owned()
    } else if dt.date_naive() == now.date_naive() - chrono::Duration::days(1) {
        "Yesterday".to_owned()
    } else {
        dt.format("%A, %d %b").to_string()
    }
}

fn paint_wall(ui: &Ui, pal: &Palette) {
    let rect = ui.max_rect();
    ui.painter().rect_filled(rect, Rounding::same(0.0), pal.wallpaper);
    let mut x = rect.left() + 30.0;
    while x < rect.right() {
        let mut y = rect.top() + 40.0;
        while y < rect.bottom() {
            ui.painter().circle_filled(
                Pos2::new(x, y),
                1.2,
                Color32::from_rgba_unmultiplied(24, 160, 110, 12),
            );
            y += 96.0;
        }
        x += 128.0;
    }
}

fn day_pill(ui: &mut Ui, pal: &Palette, label: &str) {
    ui.add_space(8.0);
    let galley = ui.fonts(|f| {
        f.layout_no_wrap(
            label.to_uppercase(),
            theme::semibold(9.0),
            pal.weak,
        )
    });
    let (row, _) = ui.allocate_exact_size(
        Vec2::new(ui.available_width(), 20.0),
        Sense::hover(),
    );
    let pill = Rect::from_center_size(
        row.center(),
        Vec2::new(galley.size().x + 22.0, 18.0),
    );
    ui.painter().rect_filled(pill, Rounding::same(8.0), pal.panel);
    ui.painter().galley(
        Pos2::new(
            pill.center().x - galley.size().x * 0.5,
            pill.center().y - galley.size().y * 0.5,
        ),
        galley,
        pal.weak,
    );
    ui.add_space(8.0);
}

fn message_bubble(
    ui: &mut Ui,
    pal: &Palette,
    msg: &MsgView,
    mine: bool,
    show_sender: bool,
) -> Option<u64> {
    let max_text = 430.0_f32;
    let text_galley = ui.fonts(|f| {
        f.layout(
            msg.text.clone(),
            theme::font(13.5),
            pal.text,
            max_text,
        )
    });
    let quote_h = if msg.reply_to.is_some() { 34.0 } else { 0.0 };
    let sender_h = if show_sender { 15.0 } else { 0.0 };
    let reactions_h = if msg.reactions.is_empty() { 0.0 } else { 19.0 };
    let w = (text_galley.size().x + 26.0).max(110.0).min(max_text + 26.0);
    let h = 9.0 + quote_h + text_galley.size().y + 17.0 + reactions_h;
    let row_h = h + sender_h + 4.0;

    let (row, resp) = ui.allocate_exact_size(
        Vec2::new(ui.available_width(), row_h),
        Sense::click(),
    );
    let x = if mine {
        row.right() - 20.0 - w
    } else {
        row.left() + 20.0
    };
    let bubble = Rect::from_min_size(
        Pos2::new(x, row.top() + sender_h + 2.0),
        Vec2::new(w, h),
    );

    if show_sender {
        ui.painter().text(
            Pos2::new(x, row.top()),
            Align2::LEFT_TOP,
            &msg.from_name,
            theme::semibold(10.0),
            Color32::from_rgb(120, 110, 180),
        );
    }

    let rounding = if mine {
        Rounding {
            nw: 13.0,
            ne: 13.0,
            sw: 13.0,
            se: 4.0,
        }
    } else {
        Rounding {
            nw: 13.0,
            ne: 13.0,
            sw: 4.0,
            se: 13.0,
        }
    };
    ui.painter().rect_filled(
        bubble.translate(Vec2::new(0.0, 1.0)),
        rounding,
        Color32::from_rgba_unmultiplied(0, 0, 0, 14),
    );
    ui.painter().rect_filled(
        bubble,
        rounding,
        if mine { pal.outgoing } else { pal.incoming },
    );
    if resp.hovered() {
        ui.painter().rect_stroke(
            bubble,
            rounding,
            Stroke::new(1.0, pal.accent),
        );
    }

    let mut top = bubble.top() + 7.0;
    if msg.reply_to.is_some() {
        let q = Rect::from_min_size(
            Pos2::new(bubble.left() + 7.0, top),
            Vec2::new(bubble.width() - 14.0, 30.0),
        );
        ui.painter().rect_filled(
            q,
            Rounding::same(6.0),
            Color32::from_rgba_unmultiplied(24, 160, 110, 22),
        );
        ui.painter().rect_filled(
            Rect::from_min_size(q.min, Vec2::new(3.0, q.height())),
            Rounding::same(2.0),
            pal.accent,
        );
        ui.painter().text(
            Pos2::new(q.left() + 8.0, q.top() + 3.0),
            Align2::LEFT_TOP,
            msg.reply_name.clone().unwrap_or_default(),
            theme::semibold(9.0),
            pal.accent_deep,
        );
        ui.painter().text(
            Pos2::new(q.left() + 8.0, q.top() + 15.0),
            Align2::LEFT_TOP,
            ellipsize(
                ui,
                msg.reply_text.as_deref().unwrap_or(""),
                theme::font(9.5),
                q.width() - 14.0,
            ),
            theme::font(9.5),
            pal.weak,
        );
        top += 34.0;
    }

    ui.painter().galley(
        Pos2::new(bubble.left() + 13.0, top),
        text_galley,
        pal.text,
    );

    if !msg.reactions.is_empty() {
        let mut grouped: Vec<(String, usize)> = Vec::new();
        for r in &msg.reactions {
            if let Some(entry) = grouped.iter_mut().find(|(e, _)| e == &r.emoji) {
                entry.1 += 1;
            } else {
                grouped.push((r.emoji.clone(), 1));
            }
        }
        let mut x = bubble.left() + 12.0;
        for (emoji, count) in grouped {
            let label = if count > 1 {
                format!("{emoji} {count}")
            } else {
                emoji
            };
            let galley = ui.fonts(|f| {
                f.layout(label.clone(), theme::font(10.0), pal.text, 80.0)
            });
            let r = Rect::from_min_size(
                Pos2::new(x, bubble.bottom() - 21.0),
                Vec2::new(galley.size().x + 10.0, 17.0),
            );
            ui.painter().rect_filled(r, Rounding::same(8.0), pal.panel);
            ui.painter().galley(
                Pos2::new(r.left() + 5.0, r.top() + 2.0),
                galley,
                pal.text,
            );
            x = r.right() + 4.0;
        }
    }

    let time = short_time(msg.at);
    ui.painter().text(
        Pos2::new(bubble.right() - 9.0, bubble.bottom() - 16.0),
        Align2::RIGHT_TOP,
        &time,
        theme::font(8.5),
        pal.weak,
    );
    if mine {
        let tick_rect = Rect::from_min_size(
            Pos2::new(bubble.right() - 52.0, bubble.bottom() - 16.0),
            Vec2::new(15.0, 10.0),
        );
        icons::ticks(
            ui.painter(),
            tick_rect,
            if msg.read { pal.tick } else { pal.weak },
            1.0,
            msg.read,
        );
    }

    if resp.hovered() {
        ui.painter().text(
            Pos2::new(bubble.right() - 4.0, bubble.top() - 14.0),
            Align2::RIGHT_BOTTOM,
            "click for actions",
            theme::font(8.5),
            pal.weak,
        );
    }

    if resp.clicked() {
        Some(msg.id)
    } else {
        None
    }
}

fn typing_bubble(ui: &mut Ui, pal: &Palette, names: &[String]) {
    ui.add_space(2.0);
    ui.horizontal(|ui| {
        ui.add_space(22.0);
        Frame::none()
            .fill(pal.incoming)
            .rounding(Rounding::same(12.0))
            .inner_margin(Margin::symmetric(12.0, 8.0))
            .show(ui, |ui| {
                ui.label(
                    RichText::new(format!("{} is typing…", names.first().cloned().unwrap_or_default()))
                        .font(theme::font(11.0))
                        .color(pal.weak),
                );
            });
    });
}
