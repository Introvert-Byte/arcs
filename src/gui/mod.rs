//! Modern desktop GUI for Arcs (egui / eframe).

mod theme;

use crate::archive::{ArchiveReader, ArchiveWriter, Manifest};
use crate::cloud::CloudClient;
use crate::config::{CloudProvider, CloudTarget};
use crate::discovery::{discover_all, DiscoveredItem, ItemKind};
use crate::restore::{execute_restore, plan_restore, RestoreOptions};
use crate::util::{current_os_arch, human_size};
use eframe::egui::{
    self, Align, Color32, Frame, Layout, Margin, RichText, Rounding, Stroke, TextEdit, Vec2,
};
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use theme::{Palette, ThemeId};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Page {
    Home,
    Discover,
    Backup,
    Restore,
    Inspect,
    Cloud,
    Help,
    Settings,
}

/// Local-only session mode (no remote server required).
#[derive(Clone, Copy, PartialEq, Eq)]
enum AuthMode {
    Guest,
    Local,
}

enum WorkerMsg {
    DiscoverDone(Result<Vec<DiscoveredItem>, String>),
    BackupDone(Result<PathBuf, String>),
    RestoreDone(Result<String, String>),
    InspectDone(Result<Manifest, String>),
    CloudDone(Result<String, String>),
}

pub struct ArcsGui {
    page: Page,
    theme: ThemeId,
    items: Vec<DiscoveredItem>,
    status: String,
    busy: bool,
    discover_filter: String,
    inspect_filter: String,
    output_path: String,
    password: String,
    encrypt: bool,
    comment: String,
    archive_path: String,
    restore_target: String,
    restore_password: String,
    dry_run: bool,
    overwrite: bool,
    manifest: Option<Manifest>,
    cloud_provider: String,
    cloud_endpoint: String,
    cloud_bucket: String,
    cloud_access: String,
    cloud_secret: String,
    cloud_remote_name: String,
    tx: Sender<WorkerMsg>,
    rx: Receiver<WorkerMsg>,
    log: Vec<String>,
    show_password: bool,
    password_confirm: String,
    /// First-run welcome + shortcut offer
    show_welcome: bool,
    shortcut_created: bool,
    logo_texture: Option<egui::TextureHandle>,
    sound_enabled: bool,
    /// Guest or local profile (no cloud account)
    auth_mode: AuthMode,
    profile_name: String,
    show_auth: bool,
    show_share: bool,
    show_rate: bool,
    rating: u8,
    rating_note: String,
    style: crate::style::StylePrefs,
    style_addons: Vec<crate::style::StyleAddon>,
}

impl Default for ArcsGui {
    fn default() -> Self {
        let (tx, rx) = mpsc::channel();
        let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
        Self {
            page: Page::Home,
            theme: ThemeId::Glass,
            items: Vec::new(),
            status: "Ready — open How to use for a guided tour".into(),
            busy: false,
            discover_filter: String::new(),
            inspect_filter: String::new(),
            output_path: home
                .join("Backups")
                .join(format!(
                    "arcs-{}.arcs",
                    chrono::Local::now().format("%Y%m%d-%H%M%S")
                ))
                .display()
                .to_string(),
            password: String::new(),
            encrypt: true,
            comment: String::new(),
            archive_path: String::new(),
            restore_target: home.display().to_string(),
            restore_password: String::new(),
            dry_run: true,
            overwrite: false,
            manifest: None,
            cloud_provider: "local".into(),
            cloud_endpoint: String::new(),
            cloud_bucket: home.join("ArcsCloud").display().to_string(),
            cloud_access: String::new(),
            cloud_secret: String::new(),
            cloud_remote_name: String::new(),
            tx,
            rx,
            log: vec![
                "Welcome to Arcs".into(),
                "1) Discover  →  2) Select  →  3) Backup  →  4) Cloud (optional)".into(),
            ],
            show_password: false,
            password_confirm: String::new(),
            show_welcome: crate::shortcut::is_first_run(),
            shortcut_created: false,
            logo_texture: None,
            sound_enabled: true,
            auth_mode: AuthMode::Guest,
            profile_name: String::new(),
            show_auth: false,
            show_share: false,
            show_rate: false,
            rating: 5,
            rating_note: String::new(),
            style: crate::style::StylePrefs::default(),
            style_addons: Vec::new(),
        }
    }
}

impl ArcsGui {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        crate::style::ensure_example_addons();
        let mut gui = Self::default();
        gui.style = crate::style::StylePrefs::load();
        gui.style_addons = crate::style::discover_addons();
        gui.theme = theme_from_style(&gui.style);
        theme::apply(&cc.egui_ctx, gui.theme);
        gui.logo_texture = load_logo_texture(&cc.egui_ctx);
        cc.egui_ctx.request_repaint();
        gui
    }

    fn palette(&self) -> Palette {
        apply_style_to_palette(self.theme.palette(), &self.style)
    }

    fn log_msg(&mut self, msg: impl Into<String>) {
        let m = msg.into();
        self.status = m.clone();
        self.log.push(m);
        if self.log.len() > 100 {
            self.log.remove(0);
        }
    }

    fn selected_count(&self) -> usize {
        self.items.iter().filter(|i| i.selected).count()
    }

    fn filtered_indices(&self) -> Vec<usize> {
        let q = self.discover_filter.to_lowercase();
        self.items
            .iter()
            .enumerate()
            .filter(|(_, it)| {
                q.is_empty()
                    || it.name.to_lowercase().contains(&q)
                    || format!("{:?}", it.kind).to_lowercase().contains(&q)
                    || it.path.to_string_lossy().to_lowercase().contains(&q)
            })
            .map(|(i, _)| i)
            .collect()
    }

    fn poll_worker(&mut self) {
        while let Ok(msg) = self.rx.try_recv() {
            self.busy = false;
            match msg {
                WorkerMsg::DiscoverDone(Ok(items)) => {
                    let n = items.len();
                    self.items = items;
                    self.log_msg(format!("OK Discovered {n} items"));
                    self.page = Page::Discover;
                }
                WorkerMsg::DiscoverDone(Err(e)) => {
                    self.log_msg(format!("ERR Discover failed: {e}"))
                }
                WorkerMsg::BackupDone(Ok(path)) => {
                    self.log_msg(format!("OK Backup written: {}", path.display()));
                    self.cloud_remote_name = path
                        .file_name()
                        .map(|s| s.to_string_lossy().to_string())
                        .unwrap_or_default();
                    self.page = Page::Home;
                }
                WorkerMsg::BackupDone(Err(e)) => self.log_msg(format!("ERR Backup failed: {e}")),
                WorkerMsg::RestoreDone(Ok(msg)) => self.log_msg(format!("OK {msg}")),
                WorkerMsg::RestoreDone(Err(e)) => {
                    self.log_msg(format!("ERR Restore failed: {e}"))
                }
                WorkerMsg::InspectDone(Ok(m)) => {
                    self.log_msg(format!(
                        "OK Loaded archive — {} files, {}",
                        m.file_count(),
                        human_size(m.total_size())
                    ));
                    self.manifest = Some(m);
                    self.page = Page::Inspect;
                }
                WorkerMsg::InspectDone(Err(e)) => {
                    self.log_msg(format!("ERR Inspect failed: {e}"))
                }
                WorkerMsg::CloudDone(Ok(msg)) => self.log_msg(format!("OK {msg}")),
                WorkerMsg::CloudDone(Err(e)) => self.log_msg(format!("ERR Cloud: {e}")),
            }
        }
    }

    fn start_discover(&mut self) {
        if self.busy {
            return;
        }
        self.busy = true;
        self.log_msg("Scanning system…");
        let tx = self.tx.clone();
        thread::spawn(move || {
            let res = discover_all().map_err(|e| e.to_string());
            let _ = tx.send(WorkerMsg::DiscoverDone(res));
        });
    }

    fn start_backup(&mut self) {
        if self.busy {
            return;
        }
        let selected: Vec<_> = self.items.iter().filter(|i| i.selected).cloned().collect();
        if selected.is_empty() {
            self.log_msg("Select at least one item in Discover");
            return;
        }
        if self.encrypt {
            if self.password.len() < 6 {
                self.log_msg("ERR Password must be at least 6 characters");
                return;
            }
            if self.password != self.password_confirm {
                self.log_msg("ERR Passwords do not match — re-enter both fields");
                return;
            }
        }
        let output = PathBuf::from(&self.output_path);
        let password = if self.encrypt {
            Some(self.password.clone())
        } else {
            None
        };
        let comment = self.comment.clone();
        let encrypt = self.encrypt;
        self.busy = true;
        self.log_msg(format!("Backing up {} items…", selected.len()));
        let tx = self.tx.clone();
        thread::spawn(move || {
            let res = (|| {
                let (os, arch) = current_os_arch();
                let mut writer = ArchiveWriter::new(&os, &arch);
                if !comment.is_empty() {
                    writer.set_comment(comment);
                }
                if encrypt {
                    let pw = password.unwrap_or_default();
                    if pw.is_empty() {
                        return Err("Password required when encryption is on".into());
                    }
                    writer.set_password(pw);
                }
                for item in &selected {
                    if item.path.exists() {
                        let prefix = kind_label(&item.kind);
                        writer
                            .add_path(&item.path, prefix)
                            .map_err(|e| e.to_string())?;
                    }
                    if let Some(pkg) = &item.package {
                        writer.add_package(pkg.clone());
                    }
                }
                if let Some(parent) = output.parent() {
                    let _ = std::fs::create_dir_all(parent);
                }
                writer.finish(&output).map_err(|e| e.to_string())?;
                Ok(output)
            })();
            let _ = tx.send(WorkerMsg::BackupDone(res));
        });
    }

    fn start_inspect(&mut self) {
        if self.busy || self.archive_path.is_empty() {
            return;
        }
        let path = PathBuf::from(&self.archive_path);
        let password = if self.restore_password.is_empty() {
            None
        } else {
            Some(self.restore_password.clone())
        };
        self.busy = true;
        self.log_msg("Reading archive…");
        let tx = self.tx.clone();
        thread::spawn(move || {
            let res = (|| {
                let reader = ArchiveReader::open(&path).map_err(|e| e.to_string())?;
                reader
                    .manifest(password.as_deref())
                    .map_err(|e| e.to_string())
            })();
            let _ = tx.send(WorkerMsg::InspectDone(res));
        });
    }

    fn start_restore(&mut self) {
        if self.busy || self.archive_path.is_empty() {
            return;
        }
        let path = PathBuf::from(&self.archive_path);
        let dry = self.dry_run;
        let opts = RestoreOptions {
            target_root: PathBuf::from(&self.restore_target),
            dry_run: dry,
            password: if self.restore_password.is_empty() {
                None
            } else {
                Some(self.restore_password.clone())
            },
            selected_paths: None,
            overwrite: self.overwrite,
        };
        self.busy = true;
        self.log_msg(if dry {
            "Planning restore (dry-run)…"
        } else {
            "Restoring files…"
        });
        let tx = self.tx.clone();
        thread::spawn(move || {
            let res = (|| {
                let reader = ArchiveReader::open(&path).map_err(|e| e.to_string())?;
                let plan = plan_restore(&reader, &opts).map_err(|e| e.to_string())?;
                let n = plan.actions.len();
                execute_restore(&reader, &plan, &opts).map_err(|e| e.to_string())?;
                Ok(if dry {
                    format!("Dry-run complete — {n} planned actions (nothing written)")
                } else {
                    format!("Restore finished — {n} actions applied")
                })
            })();
            let _ = tx.send(WorkerMsg::RestoreDone(res));
        });
    }

    fn cloud_target(&self) -> Result<CloudTarget, String> {
        let provider = match self.cloud_provider.to_lowercase().as_str() {
            "s3" => CloudProvider::S3,
            "webdav" => CloudProvider::WebDav,
            "local" => CloudProvider::Local,
            _ => return Err("Provider must be local, s3, or webdav".into()),
        };
        Ok(CloudTarget {
            provider,
            endpoint: if self.cloud_endpoint.is_empty() {
                None
            } else {
                Some(self.cloud_endpoint.clone())
            },
            bucket_or_path: self.cloud_bucket.clone(),
            access_key: if self.cloud_access.is_empty() {
                None
            } else {
                Some(self.cloud_access.clone())
            },
            secret_key: if self.cloud_secret.is_empty() {
                None
            } else {
                Some(self.cloud_secret.clone())
            },
        })
    }

    fn start_cloud_push(&mut self) {
        if self.busy {
            return;
        }
        let local = if !self.archive_path.is_empty() {
            PathBuf::from(&self.archive_path)
        } else {
            PathBuf::from(&self.output_path)
        };
        if !local.exists() {
            self.log_msg("No archive found — create a backup or pick a file first");
            return;
        }
        let target = match self.cloud_target() {
            Ok(t) => t,
            Err(e) => {
                self.log_msg(e);
                return;
            }
        };
        self.busy = true;
        self.log_msg(format!("Uploading {} …", local.display()));
        let tx = self.tx.clone();
        thread::spawn(move || {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build();
            let res = match rt {
                Ok(rt) => rt.block_on(async {
                    CloudClient::new()
                        .push(&local, &target)
                        .await
                        .map(|_| format!("Uploaded {}", local.display()))
                        .map_err(|e| e.to_string())
                }),
                Err(e) => Err(e.to_string()),
            };
            let _ = tx.send(WorkerMsg::CloudDone(res));
        });
    }

    fn start_cloud_pull(&mut self) {
        if self.busy {
            return;
        }
        if self.cloud_remote_name.is_empty() {
            self.log_msg("Enter remote file name to download");
            return;
        }
        let target = match self.cloud_target() {
            Ok(t) => t,
            Err(e) => {
                self.log_msg(e);
                return;
            }
        };
        let name = self.cloud_remote_name.clone();
        let dest = PathBuf::from(&self.output_path);
        self.busy = true;
        self.log_msg(format!("Downloading {name} …"));
        let tx = self.tx.clone();
        thread::spawn(move || {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build();
            let res = match rt {
                Ok(rt) => rt.block_on(async {
                    CloudClient::new()
                        .pull(&name, &dest, &target)
                        .await
                        .map(|_| format!("Downloaded -> {}", dest.display()))
                        .map_err(|e| e.to_string())
                }),
                Err(e) => Err(e.to_string()),
            };
            let _ = tx.send(WorkerMsg::CloudDone(res));
        });
    }
}

// ── UI helpers (no self borrow) ──────────────────────────────────

fn draw_card(ui: &mut egui::Ui, p: Palette, add: impl FnOnce(&mut egui::Ui)) {
    draw_card_r(ui, p, 16.0, true, add);
}

fn draw_card_r(
    ui: &mut egui::Ui,
    p: Palette,
    radius: f32,
    shadows: bool,
    add: impl FnOnce(&mut egui::Ui),
) {
    let mut f = Frame::none()
        .fill(p.card)
        .rounding(Rounding::same(radius))
        .inner_margin(Margin::symmetric(18.0, 16.0))
        .stroke(Stroke::new(1.0_f32, p.border));
    if shadows {
        f = f.shadow(egui::epaint::Shadow {
            offset: egui::vec2(0.0, 4.0),
            blur: 12.0,
            spread: 0.0,
            color: Color32::from_black_alpha(45),
        });
    }
    f.show(ui, add);
}

fn primary_btn(text: &str, p: Palette) -> egui::Button<'static> {
    egui::Button::new(RichText::new(text.to_owned()).color(Color32::WHITE).strong())
        .fill(p.accent)
        .rounding(Rounding::same(12.0))
        .min_size(Vec2::new(0.0, 40.0))
}

fn secondary_btn(text: &str, p: Palette) -> egui::Button<'static> {
    egui::Button::new(RichText::new(text.to_owned()).color(p.text))
        .fill(p.card_hover)
        .stroke(Stroke::new(1.0_f32, p.border))
        .rounding(Rounding::same(12.0))
        .min_size(Vec2::new(0.0, 40.0))
}

fn section_title(ui: &mut egui::Ui, p: Palette, title: &str, subtitle: &str) {
    ui.label(RichText::new(title).size(22.0).strong().color(p.text));
    if !subtitle.is_empty() {
        ui.add_space(2.0);
        ui.label(RichText::new(subtitle).size(13.5).color(p.muted));
    }
    ui.add_space(16.0);
}

fn meta_chip(ui: &mut egui::Ui, label: &str, value: &str, p: Palette) {
    ui.vertical(|ui| {
        ui.label(RichText::new(label).size(11.0).color(p.muted));
        ui.label(RichText::new(value).size(13.0).strong().color(p.text));
    });
    ui.add_space(18.0);
}

fn kind_icon(k: &ItemKind) -> &'static str {
    match k {
        ItemKind::ConfigDir => "📁",
        ItemKind::AppData => "🗂️",
        ItemKind::FlatpakApp => "📦",
        ItemKind::SnapApp => "📎",
        ItemKind::AptPackage => "📦",
        ItemKind::HomebrewFormula => "🍺",
        ItemKind::Custom => "•",
    }
}

fn kind_label(k: &ItemKind) -> &'static str {
    match k {
        ItemKind::ConfigDir => "config",
        ItemKind::AppData => "appdata",
        ItemKind::FlatpakApp => "flatpak",
        ItemKind::SnapApp => "snap",
        ItemKind::AptPackage => "apt",
        ItemKind::HomebrewFormula => "brew",
        ItemKind::Custom => "custom",
    }
}

fn kind_color(k: &ItemKind, p: Palette) -> Color32 {
    match k {
        ItemKind::ConfigDir => Color32::from_rgb(120, 180, 255),
        ItemKind::AppData => Color32::from_rgb(180, 140, 255),
        ItemKind::FlatpakApp => Color32::from_rgb(100, 200, 160),
        ItemKind::SnapApp => Color32::from_rgb(255, 180, 80),
        ItemKind::AptPackage => Color32::from_rgb(255, 120, 120),
        ItemKind::HomebrewFormula => Color32::from_rgb(200, 160, 100),
        ItemKind::Custom => p.muted,
    }
}

fn theme_icon(t: ThemeId) -> &'static str {
    match t {
        ThemeId::Light => "☀",
        ThemeId::Dark => "☾",
        ThemeId::Glass => "✧",
    }
}

fn theme_from_style(s: &crate::style::StylePrefs) -> ThemeId {
    match s.theme.to_lowercase().as_str() {
        "light" => ThemeId::Light,
        "dark" => ThemeId::Dark,
        _ => ThemeId::Glass,
    }
}

fn apply_style_to_palette(p: Palette, style: &crate::style::StylePrefs) -> Palette {
    let mut p = p;
    let [r, g, b] = style.accent;
    p.accent = Color32::from_rgb(r, g, b);
    p.accent_soft = Color32::from_rgba_unmultiplied(r, g, b, 36);
    p.accent_hover = Color32::from_rgb(r.saturating_add(30), g.saturating_add(30), b.saturating_add(20));
    if style.theme == "glass" {
        let a = (style.glass_alpha.clamp(0.05, 0.55) * 255.0) as u8;
        p.card = Color32::from_rgba_unmultiplied(255, 255, 255, a);
        p.card_hover = Color32::from_rgba_unmultiplied(255, 255, 255, a.saturating_add(20));
        p.border = Color32::from_rgba_unmultiplied(255, 255, 255, a.saturating_add(40));
    }
    p
}

impl eframe::App for ArcsGui {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.poll_worker();
        if self.busy {
            ctx.request_repaint();
        }
        let p = self.palette();

        // First-run welcome modal
        if self.show_welcome {
            self.ui_welcome_modal(ctx, p);
        }
        if self.show_share {
            self.ui_share_modal(ctx, p);
        }
        if self.show_rate {
            self.ui_rate_modal(ctx, p);
        }
        if self.show_auth {
            self.ui_auth_modal(ctx, p);
        }

        // Sidebar — Backup Studio layout
        egui::SidePanel::left("sidebar")
            .exact_width(236.0)
            .frame(
                Frame::none()
                    .fill(p.sidebar)
                    .stroke(Stroke::new(1.0_f32, p.border))
                    .inner_margin(Margin::symmetric(16.0, 14.0)),
            )
            .show(ctx, |ui| {
                // Brand with embedded logo
                ui.horizontal(|ui| {
                    if let Some(tex) = &self.logo_texture {
                        ui.add(
                            egui::Image::new(tex)
                                .fit_to_exact_size(egui::vec2(36.0, 36.0))
                                ,
                        );
                    } else {
                        ui.label(RichText::new("◆").size(22.0).color(p.accent));
                    }
                    ui.add_space(8.0);
                    ui.vertical(|ui| {
                        ui.label(RichText::new("Arcs").size(20.0).strong().color(p.text));
                        ui.label(
                            RichText::new("Backup")
                                .size(10.0)
                                .color(p.muted)
                                .strong(),
                        );
                    });
                });
                ui.add_space(18.0);
                ui.label(RichText::new("WORKSPACE").size(10.5).color(p.muted).strong());
                ui.add_space(6.0);

                for (page, label) in [
                    (Page::Home, "Overview"),
                    (Page::Discover, "Applications"),
                    (Page::Inspect, "Configuration"),
                    (Page::Backup, "Backup"),
                    (Page::Restore, "Restore"),
                    (Page::Settings, "Settings"),
                ] {
                    let selected = self.page == page;
                    let fill = if selected { p.accent_soft } else { Color32::TRANSPARENT };
                    let text = if selected { p.accent } else { p.text };
                    let label_txt = if selected {
                        format!("  {label}")
                    } else {
                        format!("  {label}")
                    };
                    let btn = egui::Button::new(
                        RichText::new(label_txt).size(14.5).color(text),
                    )
                    .fill(fill)
                    .stroke(Stroke::NONE)
                    .rounding(Rounding::same(12.0))
                    .min_size(Vec2::new(200.0, 36.0));
                    if ui.add(btn).clicked() {
                        self.page = page;
                    }
                    ui.add_space(1.0);
                }

                // Extra pages still reachable
                ui.add_space(8.0);
                ui.label(RichText::new("MORE").size(10.5).color(p.muted).strong());
                ui.add_space(4.0);
                for (page, label) in [
                    (Page::Cloud, "Cloud"),
                    (Page::Help, "Help"),
                ] {
                    let selected = self.page == page;
                    let fill = if selected { p.accent_soft } else { Color32::TRANSPARENT };
                    let text = if selected { p.accent } else { p.muted };
                    let btn = egui::Button::new(
                        RichText::new(format!("  {label}")).size(13.5).color(text),
                    )
                    .fill(fill)
                    .stroke(Stroke::NONE)
                    .rounding(Rounding::same(12.0))
                    .min_size(Vec2::new(200.0, 32.0));
                    if ui.add(btn).clicked() {
                        self.page = page;
                    }
                }
                ui.add_space(4.0);
                if ui
                    .add(
                        egui::Button::new(RichText::new("  Share").size(13.5).color(p.muted))
                            .fill(Color32::TRANSPARENT)
                            .stroke(Stroke::NONE)
                            .min_size(Vec2::new(204.0, 30.0)),
                    )
                    .clicked()
                {
                    self.show_share = true;
                }
                if ui
                    .add(
                        egui::Button::new(RichText::new("  Rate").size(13.5).color(p.muted))
                            .fill(Color32::TRANSPARENT)
                            .stroke(Stroke::NONE)
                            .min_size(Vec2::new(204.0, 30.0)),
                    )
                    .clicked()
                {
                    self.show_rate = true;
                }
                let auth_label = match self.auth_mode {
                    AuthMode::Guest => "  Account",
                    AuthMode::Local => "  Account",
                };
                if ui
                    .add(
                        egui::Button::new(RichText::new(auth_label).size(13.5).color(p.muted))
                            .fill(Color32::TRANSPARENT)
                            .stroke(Stroke::NONE)
                            .min_size(Vec2::new(204.0, 30.0)),
                    )
                    .clicked()
                {
                    self.show_auth = true;
                }

                ui.with_layout(Layout::bottom_up(Align::Min), |ui| {
                    ui.label(
                        RichText::new(format!("Arcs {}", env!("CARGO_PKG_VERSION")))
                            .size(11.0)
                            .color(p.muted),
                    );
                    // Theme cycle: Glass → Dark → Light
                    let toggle_label = match self.theme {
                        ThemeId::Glass => "✧  Glass",
                        ThemeId::Dark => "☾  Dark",
                        ThemeId::Light => "☀  Light",
                    };
                    if ui
                        .add(
                            egui::Button::new(RichText::new(toggle_label).size(12.5).color(p.muted))
                                .fill(Color32::TRANSPARENT)
                                .stroke(Stroke::NONE),
                        )
                        .clicked()
                    {
                        self.theme = match self.theme {
                            ThemeId::Glass => ThemeId::Dark,
                            ThemeId::Dark => ThemeId::Light,
                            ThemeId::Light => ThemeId::Glass,
                        };
                        theme::apply(ctx, self.theme);
                    }
                    ui.add_space(4.0);
                    let (os, arch) = current_os_arch();
                    let host = std::env::var("HOSTNAME")
                        .or_else(|_| std::env::var("COMPUTERNAME"))
                        .unwrap_or_else(|_| "this-device".into());
                    draw_card(ui, p, |ui| {
                        ui.label(RichText::new(format!("{os} · {arch}")).size(11.0).color(p.muted));
                        ui.label(RichText::new(&host).size(12.5).strong().color(p.text));
                    });
                    ui.add_space(6.0);
                });
            });

        // Status bar
        egui::TopBottomPanel::bottom("status")
            .frame(
                Frame::none()
                    .fill(p.card)
                    .stroke(Stroke::new(1.0_f32, p.border))
                    .inner_margin(Margin::symmetric(12.0, 6.0)),
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    let (icon, color) = if self.status.starts_with("OK") {
                        ("✓", p.ok)
                    } else if self.status.starts_with("ERR") {
                        ("✗", p.danger)
                    } else {
                        ("•", p.muted)
                    };
                    ui.label(RichText::new(icon).color(color));
                    ui.label(RichText::new(&self.status).size(12.5).color(p.muted));
                });
            });

        // Main content
        egui::CentralPanel::default()
            .frame(
                Frame::none()
                    .fill(p.bg)
                    .inner_margin(Margin::symmetric(18.0, 14.0)),
            )
            .show(ctx, |ui| match self.page {
                Page::Home => self.ui_home(ui, p),
                Page::Discover => self.ui_discover(ui, p),
                Page::Backup => self.ui_backup(ui, p),
                Page::Restore => self.ui_restore(ui, p),
                Page::Inspect => self.ui_inspect(ui, p),
                Page::Cloud => self.ui_cloud(ui, p),
                Page::Help => self.ui_help(ui, p),
                Page::Settings => self.ui_settings(ui, ctx, p),
            });
    }
}

impl ArcsGui {
    fn ui_welcome_modal(&mut self, ctx: &egui::Context, p: Palette) {
        let mut open = self.show_welcome;
        egui::Window::new("Welcome to Arcs")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .open(&mut open)
            .frame(
                Frame::window(&ctx.style())
                    .fill(p.card)
                    .stroke(Stroke::new(1.0_f32, p.border))
                    .rounding(Rounding::same(12.0))
                    .inner_margin(Margin::same(20.0)),
            )
            .show(ctx, |ui| {
                ui.set_min_width(420.0);
                ui.horizontal(|ui| {
                    if let Some(tex) = &self.logo_texture {
                        ui.add(egui::Image::new(tex).fit_to_exact_size(Vec2::new(64.0, 64.0)));
                        ui.add_space(12.0);
                    }
                    ui.vertical(|ui| {
                        ui.label(RichText::new("Arcs").size(26.0).strong().color(p.text));
                        ui.label(
                            RichText::new("Professional backup & migration")
                                .size(14.0)
                                .color(p.muted),
                        );
                    });
                });
                ui.add_space(16.0);
                ui.label(
                    RichText::new(
                        "Create encrypted .arcs archives of your apps, configs and data.\n\
                         Works on Windows, macOS and Linux — GUI, TUI or CLI.",
                    )
                    .size(13.5)
                    .color(p.text),
                );
                ui.add_space(12.0);
                ui.label(RichText::new("Quick start").strong().color(p.accent));
                ui.label("1. Discover  — scan apps & configs on this machine");
                ui.label("2. Select    — choose what you want to keep");
                ui.label("3. Backup    — create an encrypted .arcs archive");
                ui.label("4. Restore   — restore on the same or another machine");
                ui.add_space(16.0);

                if !self.shortcut_created {
                    ui.horizontal(|ui| {
                        if ui
                            .add(primary_btn("Create desktop shortcut", p))
                            .on_hover_text("Adds Arcs to your desktop / application menu so you can launch it anytime")
                            .clicked()
                        {
                            match crate::shortcut::create_desktop_shortcut() {
                                Ok(msg) => {
                                    self.log_msg(format!("OK {msg}"));
                                    self.shortcut_created = true;
                                    let _ = crate::shortcut::mark_first_run_done();
                                }
                                Err(e) => {
                                    self.log_msg(format!("ERR Shortcut: {e}"));
                                }
                            }
                        }
                        if ui.button("Skip for now").clicked() {
                            let _ = crate::shortcut::mark_first_run_done();
                            self.show_welcome = false;
                        }
                    });
                    if self.shortcut_created {
                        ui.label(
                            RichText::new("Shortcut created — you can close this dialog.")
                                .color(p.ok)
                                .size(13.0),
                        );
                    }
                } else {
                    if ui.add(primary_btn("Get started", p)).clicked() {
                        self.show_welcome = false;
                    }
                }
            });
        // If the user closed the window via X
        if !open {
            let _ = crate::shortcut::mark_first_run_done();
            self.show_welcome = false;
        }
    }


    fn ui_share_modal(&mut self, ctx: &egui::Context, p: Palette) {
        let mut open = self.show_share;
        egui::Window::new("Share Arcs")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .open(&mut open)
            .show(ctx, |ui| {
                ui.set_min_width(400.0);
                ui.label(RichText::new("Share Backup Studio with friends").size(15.0).strong().color(p.text));
                ui.add_space(8.0);
                ui.label(RichText::new("GitHub").size(12.0).color(p.muted));
                ui.monospace("https://github.com/introvert-byte/arcs");
                ui.add_space(6.0);
                ui.label(RichText::new("Download page").size(12.0).color(p.muted));
                ui.monospace("https://introvert-byte.github.io/arcs/");
                ui.add_space(12.0);
                ui.label(
                    RichText::new("Copy a link and send it — Arcs is free and open source.")
                        .size(13.0)
                        .color(p.muted),
                );
                ui.add_space(10.0);
                if ui.add(primary_btn("Close", p)).clicked() {
                    self.show_share = false;
                }
            });
        if !open {
            self.show_share = false;
        }
    }

    fn ui_rate_modal(&mut self, ctx: &egui::Context, p: Palette) {
        let mut open = self.show_rate;
        egui::Window::new("Rate Arcs")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .open(&mut open)
            .show(ctx, |ui| {
                ui.set_min_width(380.0);
                ui.label(RichText::new("How is Backup Studio working for you?").size(15.0).strong());
                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    for star in 1u8..=5 {
                        let filled = star <= self.rating;
                        let label = if filled { "★" } else { "☆" };
                        let color = if filled { p.warn } else { p.muted };
                        if ui
                            .add(
                                egui::Button::new(RichText::new(label).size(28.0).color(color))
                                    .fill(Color32::TRANSPARENT)
                                    .stroke(Stroke::NONE),
                            )
                            .clicked()
                        {
                            self.rating = star;
                        }
                    }
                });
                ui.add_space(8.0);
                ui.label(RichText::new("Optional note").size(12.5).color(p.muted));
                ui.add(
                    TextEdit::multiline(&mut self.rating_note)
                        .desired_width(340.0)
                        .desired_rows(3)
                        .hint_text("What should we improve?"),
                );
                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    if ui.add(primary_btn("Save feedback", p)).clicked() {
                        self.log_msg(format!(
                            "OK Thanks for rating {}/5 — feedback saved locally",
                            self.rating
                        ));
                        self.show_rate = false;
                    }
                    if ui.button("Cancel").clicked() {
                        self.show_rate = false;
                    }
                });
            });
        if !open {
            self.show_rate = false;
        }
    }

    fn ui_auth_modal(&mut self, ctx: &egui::Context, p: Palette) {
        let mut open = self.show_auth;
        egui::Window::new("Account")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .open(&mut open)
            .show(ctx, |ui| {
                ui.set_min_width(400.0);
                ui.label(
                    RichText::new("Local profiles only — no cloud account required")
                        .size(13.0)
                        .color(p.muted),
                );
                ui.add_space(12.0);

                ui.horizontal(|ui| {
                    if ui
                        .selectable_label(self.auth_mode == AuthMode::Guest, "  Guest  ")
                        .clicked()
                    {
                        self.auth_mode = AuthMode::Guest;
                        self.profile_name.clear();
                    }
                    if ui
                        .selectable_label(self.auth_mode == AuthMode::Local, "  Local profile  ")
                        .clicked()
                    {
                        self.auth_mode = AuthMode::Local;
                    }
                });
                ui.add_space(10.0);

                match self.auth_mode {
                    AuthMode::Guest => {
                        ui.label(
                            RichText::new("You are using Arcs as a guest. All data stays on this device.")
                                .size(13.5)
                                .color(p.text),
                        );
                        ui.add_space(8.0);
                        ui.label(
                            RichText::new("Tip: pick Local profile if you want a display name on Overview.")
                                .size(12.5)
                                .color(p.muted),
                        );
                    }
                    AuthMode::Local => {
                        ui.label(RichText::new("Display name").size(13.0));
                        ui.add(
                            TextEdit::singleline(&mut self.profile_name)
                                .desired_width(280.0)
                                .hint_text("e.g. Introvert"),
                        );
                        ui.add_space(6.0);
                        ui.label(
                            RichText::new("No password is sent anywhere. This is only stored on your machine.")
                                .size(12.0)
                                .color(p.muted),
                        );
                    }
                }

                ui.add_space(14.0);
                if ui.add(primary_btn("Continue", p)).clicked() {
                    if self.auth_mode == AuthMode::Local && self.profile_name.trim().is_empty() {
                        self.log_msg("ERR Enter a display name or switch to Guest");
                    } else {
                        let who = if self.auth_mode == AuthMode::Guest {
                            "Guest".into()
                        } else {
                            self.profile_name.clone()
                        };
                        self.log_msg(format!("OK Signed in as {who} (local only)"));
                        self.show_auth = false;
                    }
                }
            });
        if !open {
            self.show_auth = false;
        }
    }

    fn ui_home(&mut self, ui: &mut egui::Ui, p: Palette) {
        let mut go_discover = false;
        let mut go_backup = false;
        let mut go_restore = false;

        // Simple header
        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                ui.label(RichText::new("Overview").size(24.0).strong().color(p.text));
                ui.label(
                    RichText::new("Backup apps and configs on this device.")
                        .size(14.0)
                        .color(p.muted),
                );
            });
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if ui.add(primary_btn("New backup", p)).clicked() {
                    go_backup = true;
                }
                if ui.add(secondary_btn("Scan", p)).clicked() {
                    go_discover = true;
                }
            });
        });
        ui.add_space(16.0);

        // Status
        draw_card(ui, p, |ui| {
            if self.busy {
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label(RichText::new("Scanning…").size(14.5).color(p.text));
                });
            } else if self.items.is_empty() {
                ui.label(RichText::new("No scan yet").size(15.0).strong().color(p.text));
                ui.add_space(4.0);
                ui.label(RichText::new("1. Press Scan").size(13.5).color(p.muted));
                ui.label(RichText::new("2. Choose items in Applications").size(13.5).color(p.muted));
                ui.label(RichText::new("3. Create a backup").size(13.5).color(p.muted));
            } else {
                ui.label(
                    RichText::new(format!(
                        "{} found · {} selected",
                        self.items.len(),
                        self.selected_count()
                    ))
                    .size(15.0)
                    .strong()
                    .color(p.text),
                );
                ui.label(
                    RichText::new("Open Applications to change the selection, then Backup.")
                        .size(13.5)
                        .color(p.muted),
                );
            }
        });
        ui.add_space(12.0);

        // Three simple stats
        ui.horizontal(|ui| {
            for (label, value) in [
                ("Found", self.items.len().to_string()),
                ("Selected", self.selected_count().to_string()),
                ("Theme", self.theme.name().to_string()),
            ] {
                draw_card(ui, p, |ui| {
                    ui.set_min_width(120.0);
                    ui.label(RichText::new(label).size(12.5).color(p.muted));
                    ui.label(RichText::new(value).size(26.0).strong().color(p.text));
                });
                ui.add_space(10.0);
            }
        });
        ui.add_space(12.0);

        // Quick actions — one column, simple
        draw_card(ui, p, |ui| {
            ui.label(RichText::new("Quick actions").size(15.0).strong().color(p.text));
            ui.add_space(10.0);
            ui.horizontal(|ui| {
                if ui.add(primary_btn("Create backup", p)).clicked() {
                    go_backup = true;
                }
                if ui.add(secondary_btn("Restore", p)).clicked() {
                    go_restore = true;
                }
                if ui.add(secondary_btn("Applications", p)).clicked() {
                    go_discover = true;
                    self.page = Page::Discover;
                }
            });
        });
        ui.add_space(12.0);

        // Short activity
        draw_card(ui, p, |ui| {
            ui.label(RichText::new("Activity").size(15.0).strong().color(p.text));
            ui.add_space(6.0);
            egui::ScrollArea::vertical().max_height(140.0).show(ui, |ui| {
                if self.log.is_empty() {
                    ui.label(RichText::new("Nothing yet.").color(p.muted));
                }
                for line in self.log.iter().rev().take(8) {
                    let color = if line.starts_with("OK") {
                        p.ok
                    } else if line.starts_with("ERR") {
                        p.danger
                    } else {
                        p.muted
                    };
                    ui.label(RichText::new(line).size(13.0).color(color));
                }
            });
        });

        if go_discover {
            self.start_discover();
        }
        if go_backup {
            if self.items.is_empty() {
                self.start_discover();
            }
            self.page = Page::Backup;
        }
        if go_restore {
            self.page = Page::Restore;
        }
    }

    fn ui_discover(&mut self, ui: &mut egui::Ui, p: Palette) {
        let mut rescan = false;
        let mut select_all = false;
        let mut select_none = false;
        let mut go_backup = false;

        ui.horizontal(|ui| {
            ui.heading(RichText::new("🔍  Discover").size(24.0).color(p.text));
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if ui
                    .add_enabled(!self.busy, primary_btn("Rescan", p))
                    .clicked()
                {
                    rescan = true;
                }
                if ui.add(secondary_btn("None", p)).clicked() {
                    select_none = true;
                }
                if ui.add(secondary_btn("All", p)).clicked() {
                    select_all = true;
                }
            });
        });
        ui.label(
            RichText::new(format!(
                "{} items · {} selected · type to filter",
                self.items.len(),
                self.selected_count()
            ))
            .color(p.muted),
        );
        ui.add_space(8.0);

        ui.horizontal(|ui| {
            ui.label(RichText::new("🔎").size(16.0));
            ui.add(
                TextEdit::singleline(&mut self.discover_filter)
                    .hint_text("Search name, kind, or path…")
                    .desired_width(360.0),
            );
            if !self.discover_filter.is_empty() && ui.small_button("Clear").clicked() {
                self.discover_filter.clear();
            }
        });
        ui.add_space(8.0);

        if self.items.is_empty() {
            draw_card(ui, p, |ui| {
                ui.label(RichText::new("No items yet.").color(p.muted));
                ui.label("Click Rescan to discover apps, package managers and config folders.");
            });
        } else {
            let indices = self.filtered_indices();
            draw_card(ui, p, |ui| {
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        for &idx in &indices {
                            let item = &mut self.items[idx];
                            let icon = kind_icon(&item.kind);
                            let kind = kind_label(&item.kind);
                            let kc = kind_color(&item.kind, p);
                            ui.horizontal(|ui| {
                                ui.checkbox(&mut item.selected, "");
                                ui.label(RichText::new(icon).size(16.0));
                                ui.label(RichText::new(&item.name).size(14.0).color(p.text));
                                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                    ui.label(RichText::new(kind).size(12.0).color(kc));
                                });
                            });
                        }
                    });
            });
        }

        ui.add_space(12.0);
        if ui
            .add_enabled(
                self.selected_count() > 0 && !self.busy,
                primary_btn("Continue to Backup  ->", p),
            )
            .clicked()
        {
            go_backup = true;
        }

        if rescan {
            self.start_discover();
        }
        if select_all {
            for i in &mut self.items {
                i.selected = true;
            }
        }
        if select_none {
            for i in &mut self.items {
                i.selected = false;
            }
        }
        if go_backup {
            self.page = Page::Backup;
        }
    }

    fn ui_backup(&mut self, ui: &mut egui::Ui, p: Palette) {
        section_title(
            ui,
            p,
            "📦  Backup",
            &format!("{} items will be packaged", self.selected_count()),
        );

        let mut browse = false;
        let mut create = false;

        draw_card(ui, p, |ui| {
            ui.label(RichText::new("Output file").strong().color(p.text));
            ui.horizontal(|ui| {
                ui.add(
                    TextEdit::singleline(&mut self.output_path)
                        .desired_width(440.0)
                        .hint_text("~/Backups/backup.arcs"),
                );
                if ui.button("Browse…").clicked() {
                    browse = true;
                }
            });

            ui.add_space(10.0);
            ui.checkbox(&mut self.encrypt, "Encrypt archive (AES-256-GCM · recommended)");
            if self.encrypt {
                ui.add_space(6.0);
                ui.label(RichText::new("Password").size(13.0).color(p.text));
                ui.horizontal(|ui| {
                    ui.add(
                        TextEdit::singleline(&mut self.password)
                            .password(!self.show_password)
                            .desired_width(260.0)
                            .hint_text("Choose a strong password"),
                    );
                    if ui
                        .add(egui::Button::new(
                            if self.show_password { "Hide" } else { "Show" },
                        ))
                        .clicked()
                    {
                        self.show_password = !self.show_password;
                    }
                });
                ui.add_space(4.0);
                ui.label(RichText::new("Confirm password").size(13.0).color(p.text));
                ui.add(
                    TextEdit::singleline(&mut self.password_confirm)
                        .password(!self.show_password)
                        .desired_width(260.0)
                        .hint_text("Type the same password again"),
                );
                // Strength + match hints
                let len = self.password.chars().count();
                let (strength, scolor) = if len == 0 {
                    ("Enter a password", p.muted)
                } else if len < 6 {
                    ("Too short — use at least 6 characters", p.danger)
                } else if len < 10 {
                    ("Okay — longer is stronger", p.warn)
                } else {
                    ("Strong", p.ok)
                };
                ui.label(RichText::new(strength).size(12.0).color(scolor));
                if !self.password_confirm.is_empty() && self.password != self.password_confirm {
                    ui.label(
                        RichText::new("Passwords do not match")
                            .size(12.5)
                            .color(p.danger)
                            .strong(),
                    );
                } else if !self.password_confirm.is_empty() && self.password == self.password_confirm {
                    ui.label(RichText::new("Passwords match").size(12.5).color(p.ok));
                }
                ui.label(
                    RichText::new("We never store this password. If you lose it, the archive cannot be opened.")
                        .size(11.5)
                        .color(p.muted),
                );
            }

            ui.add_space(8.0);
            ui.label(RichText::new("Comment").color(p.muted));
            ui.add(
                TextEdit::singleline(&mut self.comment)
                    .desired_width(440.0)
                    .hint_text("e.g. laptop migration"),
            );
        });

        if browse {
            if let Some(path) = rfd::FileDialog::new()
                .add_filter("Arcs archive", &["arcs"])
                .set_file_name("backup.arcs")
                .save_file()
            {
                self.output_path = path.display().to_string();
            }
        }

        ui.add_space(14.0);
        ui.horizontal(|ui| {
            let pw_ok = !self.encrypt
                || (!self.password.is_empty()
                    && self.password.len() >= 6
                    && self.password == self.password_confirm);
            let can = self.selected_count() > 0
                && !self.busy
                && !self.output_path.is_empty()
                && pw_ok;
            if ui
                .add_enabled(can, primary_btn("Create .arcs archive", p))
                .clicked()
            {
                create = true;
            }
            if self.selected_count() == 0 {
                ui.label(
                    RichText::new("Select items in Discover first")
                        .color(p.warn)
                        .size(13.0),
                );
            }
        });
        if create {
            self.start_backup();
        }
    }

    fn ui_restore(&mut self, ui: &mut egui::Ui, p: Palette) {
        section_title(
            ui,
            p,
            "📥  Restore",
            "Open an .arcs archive and write files back (or dry-run first).",
        );

        let mut browse_file = false;
        let mut browse_dir = false;
        let mut do_restore = false;
        let mut do_inspect = false;

        draw_card(ui, p, |ui| {
            ui.label(RichText::new("Archive").strong().color(p.text));
            ui.horizontal(|ui| {
                ui.add(
                    TextEdit::singleline(&mut self.archive_path)
                        .desired_width(440.0)
                        .hint_text("path/to/file.arcs"),
                );
                if ui.button("Browse…").clicked() {
                    browse_file = true;
                }
            });

            ui.add_space(8.0);
            ui.label(RichText::new("Password (only if the archive is encrypted)").size(13.0));
            ui.horizontal(|ui| {
                ui.add(
                    TextEdit::singleline(&mut self.restore_password)
                        .password(!self.show_password)
                        .desired_width(260.0)
                        .hint_text("Archive password"),
                );
                if ui
                    .add(egui::Button::new(
                        if self.show_password { "Hide" } else { "Show" },
                    ))
                    .clicked()
                {
                    self.show_password = !self.show_password;
                }
            });

            ui.add_space(8.0);
            ui.label(RichText::new("Restore into").strong().color(p.text));
            ui.horizontal(|ui| {
                ui.add(TextEdit::singleline(&mut self.restore_target).desired_width(440.0));
                if ui.button("Browse…").clicked() {
                    browse_dir = true;
                }
            });

            ui.add_space(8.0);
            ui.checkbox(&mut self.dry_run, "Dry-run only (simulate, no writes)");
            ui.checkbox(&mut self.overwrite, "Overwrite existing files");
        });

        if browse_file {
            if let Some(path) = rfd::FileDialog::new()
                .add_filter("Arcs archive", &["arcs"])
                .pick_file()
            {
                self.archive_path = path.display().to_string();
            }
        }
        if browse_dir {
            if let Some(path) = rfd::FileDialog::new().pick_folder() {
                self.restore_target = path.display().to_string();
            }
        }

        ui.add_space(14.0);
        ui.horizontal(|ui| {
            if ui
                .add_enabled(
                    !self.busy && !self.archive_path.is_empty(),
                    primary_btn("Run restore", p),
                )
                .clicked()
            {
                do_restore = true;
            }
            if ui
                .add_enabled(
                    !self.busy && !self.archive_path.is_empty(),
                    secondary_btn("Inspect first", p),
                )
                .clicked()
            {
                do_inspect = true;
            }
        });
        if do_restore {
            self.start_restore();
        }
        if do_inspect {
            self.start_inspect();
        }
    }

    fn ui_inspect(&mut self, ui: &mut egui::Ui, p: Palette) {
        section_title(
            ui,
            p,
            "📋  Inspect",
            "Browse archive contents without extracting.",
        );

        let mut browse = false;
        let mut open = false;

        draw_card(ui, p, |ui| {
            ui.horizontal(|ui| {
                ui.add(
                    TextEdit::singleline(&mut self.archive_path)
                        .desired_width(400.0)
                        .hint_text("path/to/file.arcs"),
                );
                if ui.button("Browse…").clicked() {
                    browse = true;
                }
                if ui
                    .add_enabled(
                        !self.busy && !self.archive_path.is_empty(),
                        primary_btn("Open", p),
                    )
                    .clicked()
                {
                    open = true;
                }
            });
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.label("Password");
                ui.add(
                    TextEdit::singleline(&mut self.restore_password)
                        .password(!self.show_password)
                        .desired_width(200.0),
                );
            });
        });

        if browse {
            if let Some(path) = rfd::FileDialog::new()
                .add_filter("Arcs archive", &["arcs"])
                .pick_file()
            {
                self.archive_path = path.display().to_string();
            }
        }
        if open {
            self.start_inspect();
        }

        if let Some(ref m) = self.manifest {
            ui.add_space(12.0);
            draw_card(ui, p, |ui| {
                ui.horizontal(|ui| {
                    meta_chip(ui, "ID", &m.id.to_string()[..8], p);
                    meta_chip(
                        ui,
                        "Created",
                        &m.created_at.format("%Y-%m-%d %H:%M").to_string(),
                        p,
                    );
                    meta_chip(ui, "OS", &format!("{}/{}", m.source_os, m.source_arch), p);
                    meta_chip(ui, "Size", &human_size(m.total_size()), p);
                    meta_chip(ui, "Encrypted", if m.encrypted { "Yes" } else { "No" }, p);
                });
                if let Some(c) = &m.comment {
                    ui.add_space(6.0);
                    ui.label(RichText::new(format!("Comment: {c}")).color(p.muted));
                }
            });

            ui.add_space(10.0);
            ui.horizontal(|ui| {
                ui.label(RichText::new("🔎").size(14.0));
                ui.add(
                    TextEdit::singleline(&mut self.inspect_filter)
                        .hint_text("Filter files…")
                        .desired_width(280.0),
                );
            });
            ui.add_space(6.0);

            let q = self.inspect_filter.to_lowercase();
            draw_card(ui, p, |ui| {
                egui::ScrollArea::vertical()
                    .max_height(260.0)
                    .show(ui, |ui| {
                        for e in &m.entries {
                            if !q.is_empty()
                                && !e.archive_path.to_lowercase().contains(&q)
                                && !e.original_path.to_lowercase().contains(&q)
                            {
                                continue;
                            }
                            ui.horizontal(|ui| {
                                ui.label(
                                    RichText::new(human_size(e.size))
                                        .size(12.0)
                                        .color(p.muted)
                                        .monospace(),
                                );
                                ui.add_space(10.0);
                                ui.label(RichText::new(&e.archive_path).size(13.0).color(p.text));
                            });
                        }
                    });
            });

            if !m.packages.is_empty() {
                ui.add_space(10.0);
                ui.label(
                    RichText::new(format!("Packages ({})", m.packages.len()))
                        .strong()
                        .color(p.text),
                );
                draw_card(ui, p, |ui| {
                    for pkg in &m.packages {
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(&pkg.name).strong().color(p.text));
                            ui.label(
                                RichText::new(format!("· {}", pkg.manager))
                                    .color(p.muted)
                                    .size(12.0),
                            );
                            if let Some(v) = &pkg.version {
                                ui.label(RichText::new(v).color(p.muted).size(12.0));
                            }
                        });
                    }
                });
            }
        }
    }

    fn ui_cloud(&mut self, ui: &mut egui::Ui, p: Palette) {
        section_title(
            ui,
            p,
            "☁️  Cloud sync",
            "Push or pull .arcs archives via local folder, S3-compatible storage, or WebDAV.",
        );

        let mut browse = false;
        let mut do_push = false;
        let mut do_pull = false;

        draw_card(ui, p, |ui| {
            ui.label(RichText::new("Provider").strong().color(p.text));
            ui.horizontal(|ui| {
                for (id, label) in [
                    ("local", "Local folder"),
                    ("s3", "S3"),
                    ("webdav", "WebDAV"),
                ] {
                    let selected = self.cloud_provider == id;
                    if ui.selectable_label(selected, label).clicked() {
                        self.cloud_provider = id.into();
                    }
                }
            });

            ui.add_space(10.0);
            ui.label("Bucket / path");
            ui.add(
                TextEdit::singleline(&mut self.cloud_bucket)
                    .desired_width(420.0)
                    .hint_text("bucket name or local directory"),
            );

            if self.cloud_provider != "local" {
                ui.add_space(6.0);
                ui.label("Endpoint (optional for AWS S3)");
                ui.add(
                    TextEdit::singleline(&mut self.cloud_endpoint)
                        .desired_width(420.0)
                        .hint_text("https://s3.example.com"),
                );
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        ui.label("Access key");
                        ui.add(TextEdit::singleline(&mut self.cloud_access).desired_width(200.0));
                    });
                    ui.vertical(|ui| {
                        ui.label("Secret key");
                        ui.add(
                            TextEdit::singleline(&mut self.cloud_secret)
                                .password(true)
                                .desired_width(200.0),
                        );
                    });
                });
            }

            ui.add_space(10.0);
            ui.label(RichText::new("Local archive").color(p.muted));
            ui.horizontal(|ui| {
                ui.add(
                    TextEdit::singleline(&mut self.archive_path)
                        .desired_width(360.0)
                        .hint_text("file to upload (or last backup path)"),
                );
                if ui.button("Browse…").clicked() {
                    browse = true;
                }
            });

            ui.add_space(6.0);
            ui.label(RichText::new("Remote name (for download)").color(p.muted));
            ui.add(
                TextEdit::singleline(&mut self.cloud_remote_name)
                    .desired_width(280.0)
                    .hint_text("backup.arcs"),
            );
        });

        if browse {
            if let Some(path) = rfd::FileDialog::new()
                .add_filter("Arcs", &["arcs"])
                .pick_file()
            {
                self.archive_path = path.display().to_string();
            }
        }

        ui.add_space(14.0);
        ui.horizontal(|ui| {
            if ui
                .add_enabled(!self.busy, primary_btn("Push upload", p))
                .clicked()
            {
                do_push = true;
            }
            if ui
                .add_enabled(!self.busy, secondary_btn("Pull download", p))
                .clicked()
            {
                do_pull = true;
            }
        });
        if do_push {
            self.start_cloud_push();
        }
        if do_pull {
            self.start_cloud_pull();
        }

        ui.add_space(12.0);
        draw_card(ui, p, |ui| {
            ui.label(RichText::new("Tips").strong().color(p.text));
            ui.label(
                RichText::new("• Local: copies into the folder in Bucket / path.")
                    .size(12.5)
                    .color(p.muted),
            );
            ui.label(
                RichText::new("• S3: path-style URL; production use should add full SigV4.")
                    .size(12.5)
                    .color(p.muted),
            );
            ui.label(
                RichText::new("• WebDAV: basic auth with access/secret as user/pass.")
                    .size(12.5)
                    .color(p.muted),
            );
        });
    }

    fn ui_help(&mut self, ui: &mut egui::Ui, p: Palette) {
        section_title(ui, p, "❓  How to use Arcs", "A short guided tour.");

        egui::ScrollArea::vertical().show(ui, |ui| {
            let steps = [
                (
                    "1",
                    "Discover",
                    "Open Discover and click Rescan. Arcs finds config folders and package managers (Flatpak, Snap, APT, Homebrew, AppData…). Use the search box to filter. Check the items you care about.",
                ),
                (
                    "2",
                    "Backup",
                    "Go to Backup. Choose an output path (Browse…). Enable encryption and set a strong password. Optionally add a comment. Click Create .arcs archive.",
                ),
                (
                    "3",
                    "Inspect",
                    "Open any .arcs file to see metadata, file list and packages without extracting. Use the filter to search inside the archive.",
                ),
                (
                    "4",
                    "Restore",
                    "Pick an archive, enter the password if needed, choose a target folder. Always try Dry-run first — it shows what would be written. Then uncheck dry-run to apply.",
                ),
                (
                    "5",
                    "Cloud",
                    "Push the archive to a local sync folder, S3-compatible API, or WebDAV. Pull downloads a remote name into your output path.",
                ),
                (
                    "6",
                    "Themes & CLI",
                    "Switch Light or Dark in Settings. The same binary also supports CLI and --tui.",
                ),
            ];
            for (n, title, body) in steps {
                draw_card(ui, p, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new(n)
                                .size(18.0)
                                .strong()
                                .color(p.accent)
                                .monospace(),
                        );
                        ui.label(RichText::new(title).size(16.0).strong().color(p.text));
                    });
                    ui.add_space(4.0);
                    ui.label(RichText::new(body).size(13.5).color(p.muted));
                });
                ui.add_space(8.0);
            }

            draw_card(ui, p, |ui| {
                ui.label(RichText::new("CLI cheat sheet").strong().color(p.text));
                ui.monospace("arcs                  # open this GUI");
                ui.monospace("arcs --tui            # terminal UI");
                ui.monospace("arcs discover");
                ui.monospace("arcs backup -o out.arcs --path ~/.config --encrypt");
                ui.monospace("arcs inspect out.arcs");
                ui.monospace("arcs restore out.arcs --dry-run");
                ui.monospace("arcs search out.arcs nvim");
                ui.monospace("arcs run profiles/example.yaml");
            });
        });
    }

    fn ui_settings(&mut self, ui: &mut egui::Ui, ctx: &egui::Context, p: Palette) {
        section_title(ui, p, "Settings", "Customize style, addons, and app preferences.");

        let mut style_dirty = false;

        // —— Theme base ——
        draw_card(ui, p, |ui| {
            ui.label(RichText::new("Base theme").size(15.0).strong().color(p.text));
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                for t in ThemeId::all() {
                    let selected = self.theme == *t;
                    let label = format!("{} {}", theme_icon(*t), t.name());
                    if ui.selectable_label(selected, label).clicked() {
                        self.theme = *t;
                        self.style.theme = match t {
                            ThemeId::Light => "light".into(),
                            ThemeId::Dark => "dark".into(),
                            ThemeId::Glass => "glass".into(),
                        };
                        style_dirty = true;
                    }
                }
            });
        });
        ui.add_space(10.0);

        // —— Style controls ——
        draw_card(ui, p, |ui| {
            ui.label(RichText::new("Style").size(15.0).strong().color(p.text));
            ui.label(
                RichText::new("Tune the look. Changes apply live and can be saved.")
                    .size(12.5)
                    .color(p.muted),
            );
            ui.add_space(10.0);

            ui.label(RichText::new(format!("Corner radius  ·  {:.0}px", self.style.radius)).size(13.0));
            if ui
                .add(egui::Slider::new(&mut self.style.radius, 4.0..=28.0).show_value(false))
                .changed()
            {
                style_dirty = true;
            }

            ui.add_space(6.0);
            ui.label(
                RichText::new(format!("Glass transparency  ·  {:.0}%", self.style.glass_alpha * 100.0))
                    .size(13.0),
            );
            if ui
                .add(egui::Slider::new(&mut self.style.glass_alpha, 0.05..=0.55).show_value(false))
                .changed()
            {
                style_dirty = true;
            }

            ui.add_space(8.0);
            ui.label(RichText::new("Accent color").size(13.0));
            ui.horizontal(|ui| {
                let presets: [(&str, [u8; 3]); 6] = [
                    ("Cyan", [56, 189, 248]),
                    ("Indigo", [79, 70, 229]),
                    ("Purple", [168, 85, 247]),
                    ("Pink", [244, 114, 182]),
                    ("Emerald", [16, 185, 129]),
                    ("Amber", [245, 158, 11]),
                ];
                for (name, rgb) in presets {
                    let selected = self.style.accent == rgb;
                    if ui.selectable_label(selected, name).clicked() {
                        self.style.accent = rgb;
                        style_dirty = true;
                    }
                }
            });
            ui.add_space(6.0);
            // RGB sliders
            ui.horizontal(|ui| {
                ui.label("R");
                if ui.add(egui::Slider::new(&mut self.style.accent[0], 0..=255)).changed() {
                    style_dirty = true;
                }
            });
            ui.horizontal(|ui| {
                ui.label("G");
                if ui.add(egui::Slider::new(&mut self.style.accent[1], 0..=255)).changed() {
                    style_dirty = true;
                }
            });
            ui.horizontal(|ui| {
                ui.label("B");
                if ui.add(egui::Slider::new(&mut self.style.accent[2], 0..=255)).changed() {
                    style_dirty = true;
                }
            });

            ui.add_space(8.0);
            if ui.checkbox(&mut self.style.shadows, "Soft card shadows").changed() {
                style_dirty = true;
            }
            if ui
                .checkbox(&mut self.style.comfortable, "Comfortable spacing")
                .changed()
            {
                style_dirty = true;
            }

            ui.add_space(10.0);
            ui.horizontal(|ui| {
                if ui.add(primary_btn("Save style", p)).clicked() {
                    self.style.save();
                    self.log_msg("OK Style saved");
                }
                if ui.add(secondary_btn("Reset defaults", p)).clicked() {
                    self.style = crate::style::StylePrefs::default();
                    self.theme = theme_from_style(&self.style);
                    style_dirty = true;
                    self.log_msg("OK Style reset");
                }
            });
        });
        ui.add_space(10.0);

        // —— Style addons ——
        draw_card(ui, p, |ui| {
            ui.label(RichText::new("Style addons").size(15.0).strong().color(p.text));
            ui.label(
                RichText::new(
                    "Drop .yaml / .json packs into the addons/ folder (or ~/.config/arcs/addons). They can change theme, radius, accent, and glass.",
                )
                .size(12.5)
                .color(p.muted),
            );
            ui.add_space(8.0);
            if ui.add(secondary_btn("Refresh addons", p)).clicked() {
                crate::style::ensure_example_addons();
                self.style_addons = crate::style::discover_addons();
                self.log_msg(format!("OK {} addon(s) found", self.style_addons.len()));
            }
            ui.add_space(8.0);
            if self.style_addons.is_empty() {
                ui.label(RichText::new("No addons found yet.").color(p.muted));
            }
            for addon in self.style_addons.clone() {
                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        ui.label(RichText::new(&addon.name).size(14.0).strong().color(p.text));
                        if let Some(ref d) = addon.description {
                            ui.label(RichText::new(d).size(12.0).color(p.muted));
                        }
                    });
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        let active = self.style.active_addon.as_deref() == Some(addon.id.as_str());
                        let label = if active { "Active" } else { "Apply" };
                        if ui
                            .add_enabled(!active, primary_btn(label, p))
                            .clicked()
                        {
                            addon.apply_to(&mut self.style);
                            self.theme = theme_from_style(&self.style);
                            theme::apply(ctx, self.theme);
                            self.style.save();
                            self.log_msg(format!("OK Applied addon “{}”", addon.name));
                        }
                    });
                });
                ui.add_space(6.0);
                ui.separator();
                ui.add_space(4.0);
            }
            ui.label(
                RichText::new("Example addon fields: theme, radius, glass_alpha, accent [R,G,B], shadows, comfortable")
                    .size(11.5)
                    .color(p.muted),
            );
        });
        ui.add_space(10.0);

        // —— Shortcut + prefs ——
        draw_card(ui, p, |ui| {
            ui.label(RichText::new("App").size(15.0).strong().color(p.text));
            ui.checkbox(&mut self.show_password, "Show passwords while typing");
            ui.checkbox(&mut self.sound_enabled, "UI status feedback");
            ui.add_space(8.0);
            if ui.add(secondary_btn("Create desktop shortcut", p)).clicked() {
                match crate::shortcut::create_desktop_shortcut() {
                    Ok(msg) => {
                        self.log_msg(format!("OK {msg}"));
                        self.shortcut_created = true;
                    }
                    Err(e) => self.log_msg(format!("ERR {e}")),
                }
            }
        });

        if style_dirty {
            self.theme = theme_from_style(&self.style);
            theme::apply(ctx, self.theme);
        }
    }
}


fn load_logo_texture(ctx: &egui::Context) -> Option<egui::TextureHandle> {
    const EMBEDDED: &[u8] = include_bytes!("assets/logo.png");
    if let Ok(img) = image::load_from_memory(EMBEDDED) {
        let rgba = img.to_rgba8();
        let size = [rgba.width() as usize, rgba.height() as usize];
        let color_image = egui::ColorImage::from_rgba_unmultiplied(size, rgba.as_raw());
        return Some(ctx.load_texture("arcs_logo", color_image, Default::default()));
    }
    let candidates = [
        PathBuf::from("assets/logo.png"),
        PathBuf::from("assets/logo-128.png"),
        std::env::current_exe()
            .ok()
            .and_then(|e| e.parent().map(|p| p.join("assets/logo.png")))
            .unwrap_or_default(),
    ];
    for path in candidates {
        if path.exists() {
            if let Ok(img) = image::open(&path) {
                let rgba = img.to_rgba8();
                let size = [rgba.width() as usize, rgba.height() as usize];
                let color_image = egui::ColorImage::from_rgba_unmultiplied(size, rgba.as_raw());
                return Some(ctx.load_texture("arcs_logo", color_image, Default::default()));
            }
        }
    }
    let size = 128usize;
    let mut pixels = vec![0u8; size * size * 4];
    for y in 0..size {
        for x in 0..size {
            let i = (y * size + x) * 4;
            let cx = x as f32 - size as f32 / 2.0;
            let cy = y as f32 - size as f32 / 2.0;
            let dist = (cx * cx + cy * cy).sqrt();
            if dist < 58.0 {
                pixels[i] = 79;
                pixels[i + 1] = 70;
                pixels[i + 2] = 229;
                pixels[i + 3] = 255;
            }
        }
    }
    let color_image = egui::ColorImage::from_rgba_unmultiplied([size, size], &pixels);
    Some(ctx.load_texture("arcs_logo_fallback", color_image, Default::default()))
}

fn load_window_icon() -> Option<egui::IconData> {
    const EMBEDDED: &[u8] = include_bytes!("assets/logo-64.png");
    if let Ok(img) = image::load_from_memory(EMBEDDED) {
        let rgba = img.to_rgba8();
        return Some(egui::IconData {
            rgba: rgba.into_raw(),
            width: img.width(),
            height: img.height(),
        });
    }
    const EMBEDDED2: &[u8] = include_bytes!("assets/logo.png");
    if let Ok(img) = image::load_from_memory(EMBEDDED2) {
        let rgba = img.to_rgba8();
        return Some(egui::IconData {
            rgba: rgba.into_raw(),
            width: img.width(),
            height: img.height(),
        });
    }
    None
}

pub fn run_gui() -> eframe::Result<()> {
    let mut viewport = egui::ViewportBuilder::default()
        .with_inner_size([1120.0, 740.0])
        .with_min_inner_size([860.0, 580.0])
        .with_title("Arcs — Backup Studio");

    if let Some(icon) = load_window_icon() {
        viewport = viewport.with_icon(icon);
    }

    let options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };
    eframe::run_native(
        "Arcs",
        options,
        Box::new(|cc| Box::new(ArcsGui::new(cc))),
    )
}
