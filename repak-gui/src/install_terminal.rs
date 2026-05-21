use eframe::egui;
use egui::text::{LayoutJob, TextFormat};
use std::collections::VecDeque;
use std::io::{Result as IoResult, Write};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;
use tracing_subscriber::fmt::MakeWriter;

const MAX_TERMINAL_LINES: usize = 2_000;

static INSTALL_TERMINAL: OnceLock<InstallTerminalBuffer> = OnceLock::new();

#[derive(Clone)]
pub struct InstallTerminalBuffer {
    inner: Arc<Mutex<InstallTerminalInner>>,
}

struct InstallTerminalInner {
    lines: VecDeque<String>,
    partial: String,
}

impl InstallTerminalBuffer {
    fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(InstallTerminalInner {
                lines: VecDeque::with_capacity(MAX_TERMINAL_LINES),
                partial: String::new(),
            })),
        }
    }

    pub fn clear(&self) {
        let mut inner = self.inner.lock().unwrap();
        inner.lines.clear();
        inner.partial.clear();
    }

    fn append(&self, bytes: &[u8]) {
        let text = String::from_utf8_lossy(bytes);
        let mut inner = self.inner.lock().unwrap();
        for ch in text.chars() {
            match ch {
                '\n' => {
                    let line = std::mem::take(&mut inner.partial);
                    inner.lines.push_back(line);
                    while inner.lines.len() > MAX_TERMINAL_LINES {
                        inner.lines.pop_front();
                    }
                }
                '\r' => {
                    inner.partial.clear();
                }
                _ => inner.partial.push(ch),
            }
        }
    }

    fn snapshot(&self) -> Vec<String> {
        let inner = self.inner.lock().unwrap();
        let mut lines = inner.lines.iter().cloned().collect::<Vec<_>>();
        if !inner.partial.is_empty() {
            lines.push(inner.partial.clone());
        }
        lines
    }
}

pub fn terminal_buffer() -> InstallTerminalBuffer {
    INSTALL_TERMINAL
        .get_or_init(InstallTerminalBuffer::new)
        .clone()
}

pub fn clear_terminal() {
    terminal_buffer().clear();
}

#[derive(Clone)]
pub struct TerminalMakeWriter {
    buffer: InstallTerminalBuffer,
}

pub fn terminal_make_writer() -> TerminalMakeWriter {
    TerminalMakeWriter {
        buffer: terminal_buffer(),
    }
}

pub struct TerminalWriter {
    buffer: InstallTerminalBuffer,
}

impl<'a> MakeWriter<'a> for TerminalMakeWriter {
    type Writer = TerminalWriter;

    fn make_writer(&'a self) -> Self::Writer {
        TerminalWriter {
            buffer: self.buffer.clone(),
        }
    }
}

impl Write for TerminalWriter {
    fn write(&mut self, buf: &[u8]) -> IoResult<usize> {
        self.buffer.append(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> IoResult<()> {
        Ok(())
    }
}

#[derive(Clone)]
pub struct StripAnsiMakeWriter<M> {
    inner: M,
}

impl<M> StripAnsiMakeWriter<M> {
    pub fn new(inner: M) -> Self {
        Self { inner }
    }
}

pub struct StripAnsiWriter<W> {
    inner: W,
    state: StripState,
}

#[derive(Clone, Copy)]
enum StripState {
    Text,
    Escape,
    Csi,
}

impl<'a, M> MakeWriter<'a> for StripAnsiMakeWriter<M>
where
    M: MakeWriter<'a> + Clone + 'a,
{
    type Writer = StripAnsiWriter<M::Writer>;

    fn make_writer(&'a self) -> Self::Writer {
        StripAnsiWriter {
            inner: self.inner.make_writer(),
            state: StripState::Text,
        }
    }
}

impl<W: Write> Write for StripAnsiWriter<W> {
    fn write(&mut self, buf: &[u8]) -> IoResult<usize> {
        let mut clean = Vec::with_capacity(buf.len());
        for &byte in buf {
            match self.state {
                StripState::Text => {
                    if byte == 0x1B {
                        self.state = StripState::Escape;
                    } else {
                        clean.push(byte);
                    }
                }
                StripState::Escape => {
                    if byte == b'[' {
                        self.state = StripState::Csi;
                    } else {
                        self.state = StripState::Text;
                    }
                }
                StripState::Csi => {
                    if (0x40..=0x7E).contains(&byte) {
                        self.state = StripState::Text;
                    }
                }
            }
        }
        self.inner.write_all(&clean)?;
        Ok(buf.len())
    }

    fn flush(&mut self) -> IoResult<()> {
        self.inner.flush()
    }
}

pub fn show_install_terminal(ctx: &egui::Context, install_done: bool) -> bool {
    let viewport_options = egui::ViewportBuilder::default()
        .with_title("Install progress")
        .with_inner_size([980.0, 520.0])
        .with_min_inner_size([700.0, 320.0])
        .with_close_button(install_done);

    let mut keep_open = true;
    egui::Context::show_viewport_immediate(
        ctx,
        egui::ViewportId::from_hash_of("install_terminal_progress"),
        viewport_options,
        |ctx, class| {
            assert!(
                class == egui::ViewportClass::Immediate,
                "This egui backend doesn't support multiple viewports"
            );

            egui::CentralPanel::default()
                .frame(egui::Frame::default().fill(egui::Color32::from_rgb(10, 12, 14)))
                .show(ctx, |ui| {
                    ui.visuals_mut().override_text_color =
                        Some(egui::Color32::from_rgb(220, 225, 230));
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new("Install progress").monospace().strong());
                        if !install_done {
                            ui.spinner();
                        }
                    });
                    ui.separator();
                    egui::ScrollArea::vertical()
                        .stick_to_bottom(true)
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            let default_color = ui.style().visuals.text_color();
                            for line in terminal_buffer().snapshot() {
                                ui.label(ansi_line_to_job(&line, default_color));
                            }
                        });
                });

            if install_done {
                keep_open = false;
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            } else {
                if ctx.input(|i| i.viewport().close_requested()) {
                    ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
                }
                ctx.request_repaint_after(Duration::from_millis(100));
            }
        },
    );
    keep_open
}

fn ansi_line_to_job(line: &str, default_color: egui::Color32) -> LayoutJob {
    let mut job = LayoutJob::default();
    let mut color = default_color;
    let mut strong = false;
    let mut text = String::new();
    let mut chars = line.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch == '\u{1b}' && chars.peek() == Some(&'[') {
            chars.next();
            flush_segment(&mut job, &mut text, color, strong);
            let mut code = String::new();
            for c in chars.by_ref() {
                if c == 'm' {
                    break;
                }
                code.push(c);
            }
            apply_sgr(&code, default_color, &mut color, &mut strong);
        } else {
            text.push(ch);
        }
    }

    flush_segment(&mut job, &mut text, color, strong);
    job
}

fn flush_segment(job: &mut LayoutJob, text: &mut String, color: egui::Color32, strong: bool) {
    if text.is_empty() {
        return;
    }
    let mut format = TextFormat {
        font_id: egui::FontId::monospace(12.0),
        color,
        ..Default::default()
    };
    if strong {
        format.italics = false;
    }
    job.append(text, 0.0, format);
    text.clear();
}

fn apply_sgr(
    code: &str,
    default_color: egui::Color32,
    color: &mut egui::Color32,
    strong: &mut bool,
) {
    for part in code.split(';') {
        let value = part.parse::<u16>().unwrap_or(0);
        match value {
            0 => {
                *color = default_color;
                *strong = false;
            }
            1 => *strong = true,
            22 => *strong = false,
            30 => *color = egui::Color32::from_rgb(0, 0, 0),
            31 => *color = egui::Color32::from_rgb(205, 49, 49),
            32 => *color = egui::Color32::from_rgb(13, 188, 121),
            33 => *color = egui::Color32::from_rgb(229, 229, 16),
            34 => *color = egui::Color32::from_rgb(36, 114, 200),
            35 => *color = egui::Color32::from_rgb(188, 63, 188),
            36 => *color = egui::Color32::from_rgb(17, 168, 205),
            37 => *color = egui::Color32::from_rgb(229, 229, 229),
            39 => *color = default_color,
            90 => *color = egui::Color32::from_rgb(102, 102, 102),
            91 => *color = egui::Color32::from_rgb(241, 76, 76),
            92 => *color = egui::Color32::from_rgb(35, 209, 139),
            93 => *color = egui::Color32::from_rgb(245, 245, 67),
            94 => *color = egui::Color32::from_rgb(59, 142, 234),
            95 => *color = egui::Color32::from_rgb(214, 112, 214),
            96 => *color = egui::Color32::from_rgb(41, 184, 219),
            97 => *color = egui::Color32::from_rgb(255, 255, 255),
            _ => {}
        }
    }
}
