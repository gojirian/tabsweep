//! Tab Sweep: a window that lists open Microsoft Edge tabs with a close score
//! and closes the ones that are ticked.
//!
//! `score` does the guessing, `edge` reads and closes tabs, `theme` and
//! `fonts` are ironterm's look, `logo` draws the mark and its animation and
//! `icon` turns it into the window icon, and `paths` holds what differs by
//! operating system. This file is the window: it owns the
//! rows on screen and the worker thread that scans and closes.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod edge;
mod fonts;
mod icon;
mod logo;
mod paths;
mod score;
mod theme;

use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, Sender};

use egui::text::{LayoutJob, TextFormat, TextWrapping};
use egui::{
    pos2, vec2, Align, Color32, Frame, Layout, Margin, Rect, RichText, ScrollArea, Sense, Stroke,
    StrokeKind, TextEdit, Vec2,
};

use edge::{CloseReport, Target};
use fonts::UiWeight;
use score::{Band, Rules, Tab, Verdict};

const WINDOW_SIZE: [f32; 2] = [760.0, 720.0];
const MIN_SIZE: [f32; 2] = [520.0, 360.0];
const ROW_HEIGHT: f32 = 46.0;
const FOOTER_HEIGHT: f32 = 46.0;
const WINDOW_ICON_SIZE: usize = 64;
const HEADER_MARK: f32 = 34.0;

fn main() {
    let viewport = egui::ViewportBuilder::default()
        .with_title("Tab Sweep")
        .with_inner_size(WINDOW_SIZE)
        .with_min_inner_size(MIN_SIZE)
        .with_icon(std::sync::Arc::new(egui::IconData {
            rgba: icon::rgba(WINDOW_ICON_SIZE),
            width: WINDOW_ICON_SIZE as u32,
            height: WINDOW_ICON_SIZE as u32,
        }));
    #[cfg(windows)]
    icon::write_ico_beside_exe();
    let options = eframe::NativeOptions {
        viewport,
        centered: true,
        persist_window: false,
        ..Default::default()
    };
    let opened = eframe::run_native(
        "Tab Sweep",
        options,
        Box::new(|cc| {
            fonts::install(&cc.egui_ctx);
            theme::install(&cc.egui_ctx);
            Ok(Box::new(App::new(&cc.egui_ctx)))
        }),
    );
    if let Err(err) = opened {
        eprintln!("tabsweep: could not open a window: {err}");
        std::process::exit(1);
    }
}

/// `tabsweep --preview-closing` opens on the closing screen and stays there.
fn preview_closing() -> bool {
    std::env::args().any(|arg| arg == "--preview-closing")
}

/// What the worker thread reports back.
enum Message {
    Scanned(Result<Vec<edge::RawTab>, String>),
    Closing(CloseReport),
    Closed(Result<CloseReport, String>),
    Opened(Result<(), String>),
}

#[derive(Debug, PartialEq)]
enum Phase {
    Scanning,
    Ready,
    Closing { done: usize, total: usize },
    Failed(String),
}

/// One tab on screen.
struct Row {
    tab: Tab,
    verdict: Verdict,
    ticked: bool,
}

struct App {
    rows: Vec<Row>,
    phase: Phase,
    filter: String,
    /// What the last close did, shown until the next one.
    last_close: Option<String>,
    /// Why the last Open button press did not switch tabs, if it did not.
    open_error: Option<String>,
    sender: Sender<Message>,
    receiver: Receiver<Message>,
}

impl App {
    fn new(ctx: &egui::Context) -> Self {
        let (sender, receiver) = mpsc::channel();
        let mut app = Self {
            rows: Vec::new(),
            phase: Phase::Scanning,
            filter: String::new(),
            last_close: None,
            open_error: None,
            sender,
            receiver,
        };
        if preview_closing() {
            // Shows the closing screen without touching Edge, for looking at
            // the animation.
            app.phase = Phase::Closing { done: 4, total: 12 };
        } else {
            app.start_scan(ctx);
        }
        app
    }

    fn start_scan(&mut self, ctx: &egui::Context) {
        self.phase = Phase::Scanning;
        self.open_error = None;
        let sender = self.sender.clone();
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let _ = sender.send(Message::Scanned(edge::scan()));
            ctx.request_repaint();
        });
    }

    fn start_close(&mut self, ctx: &egui::Context) {
        let targets: Vec<Target> = self
            .rows
            .iter()
            .filter(|row| row.ticked)
            .map(|row| Target {
                window: row.tab.window,
                index: row.tab.index,
                title: row.tab.title.clone(),
            })
            .collect();
        if targets.is_empty() {
            return;
        }
        self.phase = Phase::Closing {
            done: 0,
            total: targets.len(),
        };
        let sender = self.sender.clone();
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let result = edge::close(&targets, |report| {
                let _ = sender.send(Message::Closing(report.clone()));
                ctx.request_repaint();
            });
            let _ = sender.send(Message::Closed(result));
            ctx.request_repaint();
        });
    }

    /// Show `target` in Edge. The list stays as it is; only Edge changes.
    fn start_open(&mut self, ctx: &egui::Context, target: Target) {
        self.open_error = None;
        let sender = self.sender.clone();
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let _ = sender.send(Message::Opened(edge::focus(&target)));
            ctx.request_repaint();
        });
    }

    fn take_messages(&mut self, ctx: &egui::Context) {
        while let Ok(message) = self.receiver.try_recv() {
            match message {
                Message::Scanned(Ok(raw)) => {
                    self.rows = build_rows(&raw, &load_rules());
                    self.phase = Phase::Ready;
                }
                Message::Scanned(Err(reason)) => self.phase = Phase::Failed(reason),
                Message::Closing(report) => {
                    if let Phase::Closing { done, .. } = &mut self.phase {
                        *done = report.closed + report.skipped;
                    }
                }
                Message::Closed(Ok(report)) => {
                    self.last_close = Some(close_summary(&report));
                    // The strip has changed, so the indexes on screen are stale.
                    self.start_scan(ctx);
                }
                Message::Closed(Err(reason)) => self.phase = Phase::Failed(reason),
                Message::Opened(result) => self.open_error = result.err(),
            }
        }
    }

    fn busy(&self) -> bool {
        matches!(self.phase, Phase::Scanning | Phase::Closing { .. })
    }

    fn draw_header(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            let (mark, _) = ui.allocate_exact_size(Vec2::splat(HEADER_MARK), Sense::hover());
            logo::paint(
                ui.painter(),
                mark.center(),
                HEADER_MARK * 0.44,
                logo::Sweep::resting(),
            );
            ui.add_space(4.0);
            ui.vertical(|ui| {
                ui.label(
                    RichText::new("Tab Sweep")
                        .font(fonts::ui_font(17.0, UiWeight::Semibold))
                        .color(theme::TEXT_PRIMARY),
                );
                ui.label(
                    RichText::new(self.summary())
                        .font(fonts::ui_font(12.0, UiWeight::Regular))
                        .color(theme::TEXT_MUTED),
                );
            });
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let busy = self.busy();
                if action_button(ui, "Rescan", false, !busy) {
                    self.start_scan(ui.ctx());
                }
                if action_button(ui, "Rules", false, true) {
                    open_rules_file();
                }
            });
        });
    }

    fn summary(&self) -> String {
        match &self.phase {
            Phase::Scanning if self.rows.is_empty() => "Reading Edge's tabs".to_string(),
            Phase::Failed(_) => "The scan failed".to_string(),
            _ => {
                let windows = count_windows(&self.rows);
                let suggested = self
                    .rows
                    .iter()
                    .filter(|row| row.verdict.band() == Band::Close)
                    .count();
                format!(
                    "{} tabs in {} {} \u{b7} {} suggested for closing",
                    self.rows.len(),
                    windows,
                    if windows == 1 { "window" } else { "windows" },
                    suggested
                )
            }
        }
    }

    fn draw_filter(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            let edit_width = (ui.available_width() - 190.0).max(120.0);
            Frame::new()
                .fill(theme::CANVAS)
                .stroke(Stroke::new(1.0, theme::BORDER_DEFAULT))
                .corner_radius(theme::RADIUS)
                .inner_margin(Margin::symmetric(8, 6))
                .show(ui, |ui| {
                    ui.add(
                        TextEdit::singleline(&mut self.filter)
                            .hint_text("Filter by title, e.g. linkedin")
                            .desired_width(edit_width)
                            .frame(Frame::NONE)
                            .font(fonts::ui_font(13.0, UiWeight::Regular))
                            .text_color(theme::TEXT_PRIMARY),
                    );
                });
            let enabled = !self.busy();
            if action_button(ui, "Tick shown", false, enabled) {
                self.set_shown(true);
            }
            if action_button(ui, "Untick", false, enabled) {
                self.set_shown(false);
            }
        });
    }

    fn shows(&self, row: &Row) -> bool {
        let filter = self.filter.trim().to_lowercase();
        filter.is_empty() || row.tab.title.to_lowercase().contains(&filter)
    }

    fn set_shown(&mut self, ticked: bool) {
        let filter = self.filter.trim().to_lowercase();
        for row in &mut self.rows {
            if filter.is_empty() || row.tab.title.to_lowercase().contains(&filter) {
                row.ticked = ticked;
            }
        }
    }

    fn draw_list(&mut self, ui: &mut egui::Ui) {
        if let Phase::Failed(reason) = &self.phase {
            empty_line(ui, reason);
            return;
        }
        if self.rows.is_empty() {
            empty_line(ui, "No Edge windows are open.");
            return;
        }

        let shown: Vec<usize> = (0..self.rows.len())
            .filter(|at| self.shows(&self.rows[*at]))
            .collect();
        if shown.is_empty() {
            empty_line(ui, "No tab matches the filter.");
            return;
        }

        let enabled = !self.busy();
        let mut to_open = None;
        for band in [Band::Close, Band::Maybe, Band::Keep] {
            let in_band: Vec<usize> = shown
                .iter()
                .copied()
                .filter(|at| self.rows[*at].verdict.band() == band)
                .collect();
            if in_band.is_empty() {
                continue;
            }
            section_heading(ui, band_label(band), in_band.len());
            for at in in_band {
                if draw_row(ui, &mut self.rows[at], enabled) {
                    to_open = Some(at);
                }
            }
            ui.add_space(8.0);
        }
        if let Some(at) = to_open {
            let tab = &self.rows[at].tab;
            let target = Target {
                window: tab.window,
                index: tab.index,
                title: tab.title.clone(),
            };
            self.start_open(ui.ctx(), target);
        }
    }

    fn draw_footer(&mut self, ui: &mut egui::Ui) {
        let ticked: Vec<&Row> = self.rows.iter().filter(|row| row.ticked).collect();
        let count = ticked.len();
        let memory: u32 = ticked.iter().filter_map(|row| row.tab.memory_mb).sum();
        let (text, color) = self.footer_text(count, memory);
        let busy = self.busy();

        ui.horizontal(|ui| {
            ui.set_height(FOOTER_HEIGHT - 12.0);
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let label = match count {
                    0 => "Close tabs".to_string(),
                    1 => "Close 1 tab".to_string(),
                    many => format!("Close {many} tabs"),
                };
                if action_button(ui, &label, true, count > 0 && !busy) {
                    self.start_close(ui.ctx());
                }
                ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
                    ui.add(
                        egui::Label::new(
                            RichText::new(text)
                                .font(fonts::ui_font(12.0, UiWeight::Regular))
                                .color(color),
                        )
                        .truncate(),
                    );
                });
            });
        });
    }

    fn footer_text(&self, count: usize, memory: u32) -> (String, Color32) {
        if let (Phase::Ready, Some(reason)) = (&self.phase, &self.open_error) {
            return (format!("Could not open it: {reason}"), theme::WARN_TEXT);
        }
        match &self.phase {
            // The scene above the footer says what is happening.
            Phase::Closing { .. } | Phase::Scanning => (String::new(), theme::TEXT_MUTED),
            Phase::Failed(_) => (String::new(), theme::TEXT_MUTED),
            Phase::Ready if count == 0 => (
                self.last_close
                    .clone()
                    .unwrap_or_else(|| "Nothing ticked.".to_string()),
                theme::TEXT_MUTED,
            ),
            Phase::Ready => {
                let freed = if memory > 0 {
                    format!(" \u{b7} frees about {}", score::memory_text(memory))
                } else {
                    String::new()
                };
                (
                    format!(
                        "{count} ticked{freed} \u{b7} {} in Edge reopens a tab",
                        paths::reopen_tab_shortcut()
                    ),
                    theme::TEXT_SECONDARY,
                )
            }
        }
    }
}

impl eframe::App for App {
    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        theme::SURFACE_CARD.to_normalized_gamma_f32()
    }

    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.take_messages(ctx);
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let frame = Frame::NONE.fill(theme::SURFACE_CARD).inner_margin(16);
        egui::CentralPanel::default().frame(frame).show(ui, |ui| {
            self.draw_header(ui);
            ui.add_space(8.0);
            self.draw_filter(ui);
            ui.add_space(8.0);
            hairline(ui);

            let list_height = (ui.available_height() - FOOTER_HEIGHT).max(60.0);
            match self.phase {
                Phase::Scanning => {
                    let area = scene_area(ui, list_height);
                    draw_loading(ui, area);
                }
                Phase::Closing { done, total } => {
                    let area = scene_area(ui, list_height);
                    draw_closing(ui, area, done, total);
                }
                Phase::Ready | Phase::Failed(_) => {
                    ScrollArea::vertical()
                        .max_height(list_height)
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            ui.add_space(8.0);
                            self.draw_list(ui);
                        });
                }
            }

            hairline(ui);
            ui.add_space(4.0);
            self.draw_footer(ui);
        });
    }
}

/// Score the scan and order it: highest score first, then by place in the
/// strip. Tabs in the close band start ticked.
fn build_rows(raw: &[edge::RawTab], rules: &Rules) -> Vec<Row> {
    let tabs: Vec<Tab> = raw
        .iter()
        .map(|tab| score::parse_name(tab.window, tab.index, &tab.name, tab.active))
        .collect();
    let verdicts = score::score_all(&tabs, rules);
    let mut rows: Vec<Row> = tabs
        .into_iter()
        .zip(verdicts)
        .map(|(tab, verdict)| Row {
            ticked: verdict.band() == Band::Close,
            tab,
            verdict,
        })
        .collect();
    rows.sort_by(|left, right| {
        right
            .verdict
            .score
            .cmp(&left.verdict.score)
            .then(left.tab.window.cmp(&right.tab.window))
            .then(left.tab.index.cmp(&right.tab.index))
    });
    rows
}

fn count_windows(rows: &[Row]) -> usize {
    let mut windows: Vec<i64> = rows.iter().map(|row| row.tab.window).collect();
    windows.sort_unstable();
    windows.dedup();
    windows.len()
}

fn close_summary(report: &CloseReport) -> String {
    let mut text = match report.closed {
        1 => "Closed 1 tab".to_string(),
        count => format!("Closed {count} tabs"),
    };
    if report.skipped > 0 {
        text.push_str(&format!(", skipped {}", report.skipped));
    }
    if let Some(reason) = &report.stopped {
        text.push_str(&format!(". Stopped: {reason}"));
    }
    text.push('.');
    text
}

fn band_label(band: Band) -> &'static str {
    match band {
        Band::Close => "CLOSE",
        Band::Maybe => "WORTH A LOOK",
        Band::Keep => "KEEP",
    }
}

fn band_color(band: Band) -> Color32 {
    match band {
        Band::Close => theme::DANGER,
        Band::Maybe => theme::WARN_TEXT,
        Band::Keep => theme::TEXT_FAINT,
    }
}

// --- rules file -----------------------------------------------------------

const RULES_TEMPLATE: &str = "\
# Tab Sweep rules. One per line, matched against tab titles, any case.
#
#   close: gadget shop   a title containing this is always suggested
#   keep: acme portal    a title containing this is never suggested
#
# Press Rescan in Tab Sweep after saving.
";

fn rules_path() -> Option<PathBuf> {
    Some(paths::config_dir()?.join("rules.txt"))
}

fn load_rules() -> Rules {
    rules_path()
        .and_then(|path| std::fs::read_to_string(path).ok())
        .map(|text| Rules::parse(&text))
        .unwrap_or_default()
}

/// Open `rules.txt` in a text editor, writing the commented template first if the
/// file is not there yet.
fn open_rules_file() {
    let Some(path) = rules_path() else {
        return;
    };
    if !path.exists() {
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let _ = std::fs::write(&path, RULES_TEMPLATE);
    }
    paths::open_in_text_editor(&path);
}

// --- drawing ----------------------------------------------------------------

fn hairline(ui: &mut egui::Ui) {
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 1.0), Sense::hover());
    ui.painter().rect_filled(rect, 0.0, theme::BORDER_DEFAULT);
}

/// Take the space the list would have, for a scene drawn in its place.
fn scene_area(ui: &mut egui::Ui, height: f32) -> Rect {
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), height), Sense::hover());
    rect
}

/// Seconds since the window opened, and a request to draw again straight
/// away, which is what keeps a scene moving.
fn animation_time(ui: &egui::Ui) -> f64 {
    ui.ctx().request_repaint();
    ui.input(|input| input.time)
}

/// The mark's radius for a scene: large, but never more than the space allows.
fn scene_radius(area: Rect, wanted: f32, room_needed: f32) -> f32 {
    let room = area.width().min(area.height() - 70.0) / 2.0;
    wanted.min(room / room_needed).max(12.0)
}

fn scene_caption(ui: &egui::Ui, top: egui::Pos2, title: &str, detail: &str, detail_color: Color32) {
    ui.painter().text(
        top,
        egui::Align2::CENTER_TOP,
        title,
        fonts::ui_font(15.0, UiWeight::Semibold),
        theme::TEXT_PRIMARY,
    );
    ui.painter().text(
        top + vec2(0.0, 24.0),
        egui::Align2::CENTER_TOP,
        detail,
        fonts::ui_font(12.0, UiWeight::Regular),
        detail_color,
    );
}

/// The loading screen: the mark with its arm turning while Edge is read.
fn draw_loading(ui: &egui::Ui, area: Rect) {
    let seconds = animation_time(ui);
    let radius = scene_radius(area, 46.0, 1.1);
    let center = area.center() - vec2(0.0, 30.0);
    logo::paint(ui.painter(), center, radius, logo::Sweep::at(seconds));
    scene_caption(
        ui,
        center + vec2(0.0, radius + 22.0),
        "Reading Edge's tabs",
        "This takes a few seconds.",
        theme::TEXT_MUTED,
    );
}

/// The closing screen: the turning mark inside a ring that fills as tabs
/// close, with small bars drifting away from it.
fn draw_closing(ui: &egui::Ui, area: Rect, done: usize, total: usize) {
    let seconds = animation_time(ui);
    let radius = scene_radius(area, 58.0, 2.4);
    let center = area.center() - vec2(0.0, 30.0);
    let painter = ui.painter().with_clip_rect(area);

    logo::paint_swept_tabs(&painter, center, radius, seconds);
    let ring = radius * 1.3;
    painter.circle_stroke(center, ring, Stroke::new(3.0, theme::BORDER_DEFAULT));
    #[allow(clippy::cast_precision_loss)]
    let fraction = if total == 0 {
        0.0
    } else {
        done as f32 / total as f32
    };
    // Blue, the colour for things that measure, so it does not read as part
    // of the copper mark.
    logo::paint_arc(
        &painter,
        center,
        ring,
        fraction,
        Stroke::new(3.0, theme::INFO),
    );
    logo::paint(&painter, center, radius, logo::Sweep::at(seconds));

    let current = (done + 1).min(total);
    scene_caption(
        ui,
        center + vec2(0.0, radius * 2.35 + 14.0),
        &format!("Closing {current} of {total}"),
        "Leave Edge alone until this finishes.",
        theme::WARN_TEXT,
    );
}

fn empty_line(ui: &mut egui::Ui, text: &str) {
    ui.add_space(24.0);
    ui.label(
        RichText::new(text)
            .font(fonts::ui_font(13.0, UiWeight::Regular))
            .color(theme::TEXT_MUTED),
    );
}

fn section_heading(ui: &mut egui::Ui, label: &str, count: usize) {
    ui.horizontal(|ui| {
        ui.add_space(8.0);
        ui.label(
            RichText::new(label)
                .font(fonts::ui_font(11.0, UiWeight::Semibold))
                .color(theme::TEXT_MUTED),
        );
        ui.label(
            RichText::new(count.to_string())
                .font(fonts::mono_font(11.0))
                .color(theme::TEXT_FAINT),
        );
    });
}

/// A row: tick box, score, title over its reasons, an Open button, and memory
/// on the right. Clicking anywhere else on it flips the tick. Returns whether
/// Open was pressed.
fn draw_row(ui: &mut egui::Ui, row: &mut Row, enabled: bool) -> bool {
    let size = vec2(ui.available_width(), ROW_HEIGHT);
    let sense = if enabled {
        Sense::click()
    } else {
        Sense::hover()
    };
    let (rect, response) = ui.allocate_exact_size(size, sense);
    if !ui.is_rect_visible(rect) {
        return false;
    }
    let memory_width = 76.0;
    let open_rect = Rect::from_center_size(
        pos2(rect.right() - memory_width - 36.0, rect.center().y),
        vec2(52.0, 26.0),
    );
    // Added after the row, so it is on top and takes its own clicks.
    let open = ui.interact(
        open_rect,
        ui.scope_id().with(("open", row.tab.window, row.tab.index)),
        sense,
    );
    let on_open = response
        .interact_pointer_pos()
        .is_some_and(|pointer| open_rect.contains(pointer));
    if response.clicked() && !on_open {
        row.ticked = !row.ticked;
    }
    if enabled && response.hovered() {
        ui.painter().rect_filled(rect, 6.0, theme::SURFACE_HOVER);
    }

    let middle = rect.center().y;
    paint_tick_box(ui, pos2(rect.left() + 18.0, middle), row.ticked);

    let band = row.verdict.band();
    ui.painter().text(
        pos2(rect.left() + 58.0, middle),
        egui::Align2::RIGHT_CENTER,
        row.verdict.score.to_string(),
        fonts::mono_font(12.0),
        band_color(band),
    );

    let memory = row
        .tab
        .memory_mb
        .map(score::memory_text)
        .unwrap_or_default();
    paint_open_button(ui, open_rect, enabled, open.hovered());
    ui.painter().text(
        pos2(rect.right() - 10.0, middle),
        egui::Align2::RIGHT_CENTER,
        memory,
        fonts::mono_font(11.0),
        theme::TEXT_FAINT,
    );

    let text_left = rect.left() + 72.0;
    let text_right = open_rect.left() - 10.0;
    let title_color = if band == Band::Keep {
        theme::TEXT_SECONDARY
    } else {
        theme::TEXT_PRIMARY
    };
    let title = if row.tab.title.is_empty() {
        "(no title)"
    } else {
        row.tab.title.as_str()
    };
    paint_ellipsis(
        ui,
        Rect::from_min_max(
            pos2(text_left, rect.top() + 5.0),
            pos2(text_right, middle + 1.0),
        ),
        title,
        fonts::ui_font(13.0, UiWeight::Medium),
        title_color,
    );
    paint_ellipsis(
        ui,
        Rect::from_min_max(
            pos2(text_left, middle + 1.0),
            pos2(text_right, rect.bottom() - 5.0),
        ),
        &detail_text(row),
        fonts::ui_font(11.5, UiWeight::Regular),
        theme::TEXT_MUTED,
    );
    open.on_hover_text("Show this tab in Edge").clicked()
}

/// The small outlined button that shows a row's tab in Edge.
fn paint_open_button(ui: &egui::Ui, rect: Rect, enabled: bool, hovered: bool) {
    let lit = enabled && hovered;
    if lit {
        ui.painter().rect_filled(rect, 6.0, theme::SURFACE_HOVER);
    }
    let border = if lit {
        theme::BORDER_HOVER
    } else {
        theme::BORDER_DEFAULT
    };
    ui.painter()
        .rect_stroke(rect, 6.0, Stroke::new(1.0, border), StrokeKind::Inside);
    let color = if enabled {
        theme::TEXT_SECONDARY
    } else {
        theme::TEXT_FAINT
    };
    ui.painter().text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        "Open",
        fonts::ui_font(12.0, UiWeight::Medium),
        color,
    );
}

/// The reasons, with the tab group named first when there is one.
fn detail_text(row: &Row) -> String {
    let mut parts: Vec<String> = Vec::new();
    if let Some(group) = &row.tab.group {
        parts.push(format!("group {group}"));
    }
    parts.extend(
        row.verdict
            .reasons
            .iter()
            .filter(|reason| reason.as_str() != "in a tab group")
            .cloned(),
    );
    if parts.is_empty() {
        "nothing against it".to_string()
    } else {
        parts.join(" \u{b7} ")
    }
}

/// Painted, since the chrome avoids symbol glyphs.
fn paint_tick_box(ui: &egui::Ui, center: egui::Pos2, ticked: bool) {
    let rect = Rect::from_center_size(center, Vec2::splat(16.0));
    if !ticked {
        ui.painter().rect_stroke(
            rect,
            4.0,
            Stroke::new(1.0, theme::BORDER_HOVER),
            StrokeKind::Inside,
        );
        return;
    }
    ui.painter().rect_filled(rect, 4.0, theme::ACCENT);
    let points = vec![
        center + vec2(-4.0, 0.5),
        center + vec2(-1.2, 3.2),
        center + vec2(4.0, -3.0),
    ];
    ui.painter().add(egui::Shape::line(
        points,
        Stroke::new(1.75, theme::ACCENT_ON),
    ));
}

/// One line of text, cut with an ellipsis where it would leave `rect`.
fn paint_ellipsis(ui: &egui::Ui, rect: Rect, text: &str, font: egui::FontId, color: Color32) {
    if rect.width() < 4.0 || rect.height() < 4.0 {
        return;
    }
    let mut job = LayoutJob {
        wrap: TextWrapping {
            max_width: rect.width(),
            max_rows: 1,
            break_anywhere: true,
            overflow_character: Some('\u{2026}'),
        },
        ..Default::default()
    };
    job.append(
        text,
        0.0,
        TextFormat {
            font_id: font,
            color,
            ..Default::default()
        },
    );
    let galley = ui.ctx().fonts_mut(|fonts| fonts.layout_job(job));
    let y = rect.center().y - galley.size().y * 0.5;
    ui.painter()
        .with_clip_rect(rect)
        .galley(pos2(rect.min.x, y), galley, color);
}

/// Copper with dark text for the one main action, outlined otherwise.
fn action_button(ui: &mut egui::Ui, label: &str, primary: bool, enabled: bool) -> bool {
    let (fill, color) = match (primary, enabled) {
        (true, true) => (theme::ACCENT, theme::ACCENT_ON),
        (true, false) => (theme::SURFACE_RAISED, theme::TEXT_FAINT),
        (false, true) => (Color32::TRANSPARENT, theme::TEXT_PRIMARY),
        (false, false) => (Color32::TRANSPARENT, theme::TEXT_FAINT),
    };
    let text = RichText::new(label)
        .font(fonts::ui_font(13.0, UiWeight::Medium))
        .color(color);
    ui.add_enabled(
        enabled,
        egui::Button::new(text).fill(fill).min_size(vec2(0.0, 30.0)),
    )
    .clicked()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn raw(index: usize, name: &str) -> edge::RawTab {
        edge::RawTab {
            window: 1,
            index,
            name: name.to_string(),
            active: false,
        }
    }

    #[test]
    fn rows_are_ordered_by_score_and_the_close_band_starts_ticked() {
        let rows = build_rows(
            &[
                raw(0, "Incidents - Status Board"),
                raw(1, "example.com - Network error"),
                raw(2, "laptop reviews - Google Search"),
            ],
            &Rules::default(),
        );
        let titles: Vec<&str> = rows.iter().map(|row| row.tab.title.as_str()).collect();
        assert_eq!(
            titles,
            [
                "example.com - Network error",
                "laptop reviews - Google Search",
                "Incidents - Status Board"
            ]
        );
        let ticked: Vec<bool> = rows.iter().map(|row| row.ticked).collect();
        assert_eq!(ticked, [true, false, false]);
    }

    #[test]
    fn the_close_summary_says_what_was_skipped_and_why_it_stopped() {
        let report = CloseReport {
            closed: 3,
            skipped: 1,
            stopped: Some("a tab did not close".to_string()),
        };
        assert_eq!(
            close_summary(&report),
            "Closed 3 tabs, skipped 1. Stopped: a tab did not close."
        );
        let one = CloseReport {
            closed: 1,
            ..CloseReport::default()
        };
        assert_eq!(close_summary(&one), "Closed 1 tab.");
    }
}
