// dios-shot — headless screenshot renderer for the DIOS Downloader UI.
//
// Renders the real .slint windows with mock data through Slint's software
// renderer and saves a PNG. Design tool only; never shipped with the app.
//
//     cargo run -p dios --features shot --bin dios-shot -- main out.png 0 1180 640
//     args: <main|add|details> <out.png> [theme 0..8] [w] [h]

slint::include_modules!();

use slint::platform::software_renderer::{MinimalSoftwareWindow, RepaintBufferType, TargetPixel};
use slint::platform::{Platform, PlatformError};
use std::rc::Rc;

struct ShotPlatform {
    win: Rc<MinimalSoftwareWindow>,
}

impl Platform for ShotPlatform {
    fn create_window_adapter(&self) -> Result<Rc<dyn slint::platform::WindowAdapter>, PlatformError> {
        Ok(self.win.clone())
    }
    fn run_event_loop(&self) -> Result<(), PlatformError> {
        Ok(())
    }
}

// ---------------------------------------------------------------- mock data

fn row(id: i32, name: &str, kind: &str, category: &str, queue: &str, size: &str, status: &str,
       progress: f32, eta: &str, speed: &str, last_try: &str, description: &str,
       added: &str, sel: bool, save_to: &str, state: i32) -> DownloadRow {
    DownloadRow {
        id,
        name: name.into(),
        kind: kind.into(),
        category: category.into(),
        queue: queue.into(),
        size: size.into(),
        status: status.into(),
        progress,
        eta: eta.into(),
        speed: speed.into(),
        last_try: last_try.into(),
        description: description.into(),
        added: added.into(),
        sel,
        save_to: save_to.into(),
        referer: "".into(),
        parent: "".into(),
        state,
    }
}

fn mock_rows() -> Vec<DownloadRow> {
    vec![
        row(1, "ubuntu-24.04.3-desktop-amd64.iso", "disc", "Programs", "Main", "5.8 GB",
            "44.3%", 0.443, "00:07:12", "11.4 MB/s", "Today 14:22", "LTS desktop image",
            "Today 14:02", false, "C:\\Users\\me\\Downloads", 1),
        row(2, "Big.Buck.Bunny.4K.2160p.WEB-DL.mkv", "video", "Video", "Main", "1.42 GB",
            "87.4%", 0.874, "00:00:41", "8.2 MB/s", "Today 14:31", "",
            "Today 14:19", true, "C:\\Users\\me\\Videos", 1),
        row(3, "android-studio-2025.1.2-windows.exe", "exe", "Programs", "Main", "1.18 GB",
            "51.2%", 0.512, "—", "0 KB/s", "Today 13:40", "paused by user",
            "Today 11:55", false, "C:\\Users\\me\\Downloads", 2),
        row(4, "flutter_windows_3.32.0-stable.zip", "archive", "Compressed", "Main", "984 MB",
            "0%", 0.0, "—", "", "Yesterday 22:10", "server returned 403",
            "Yesterday 22:04", false, "C:\\Users\\me\\Downloads", 4),
        row(5, "annual-report-2025-en.pdf", "pdf", "Documents", "", "8.4 MB",
            "Queued", 0.0, "—", "", "—", "",
            "Today 14:33", false, "C:\\Users\\me\\Documents", 0),
        row(6, "summer-holiday-4k-photos-pack.jpg", "image", "Images", "Nightly", "24.6 MB",
            "100%", 1.0, "00:00:00", "6.1 MB/s", "Today 09:12", "",
            "Today 09:11", false, "C:\\Users\\me\\Pictures", 3),
        row(7, "deep-focus-playlist-vol.3.mp3", "audio", "Music", "", "12.1 MB",
            "100%", 1.0, "00:00:00", "5.4 MB/s", "Today 08:47", "",
            "Today 08:45", false, "C:\\Users\\me\\Music", 3),
        row(8, "meeting-notes-12-oct.txt", "text", "Documents", "", "4 KB",
            "100%", 1.0, "00:00:00", "1.2 MB/s", "12 Oct 10:31", "",
            "12 Oct 10:30", false, "C:\\Users\\me\\Documents", 3),
        row(9, "godot-4.5-stable-win64.exe", "exe", "Programs", "Nightly", "112 MB",
            "100%", 1.0, "00:00:00", "9.8 MB/s", "11 Oct 18:04", "game engine",
            "11 Oct 18:00", false, "C:\\Users\\me\\Downloads", 3),
        row(10, "blender-4.5-linux-x64.tar.xz", "archive", "Compressed", "", "321 MB",
            "100%", 1.0, "00:00:00", "7.3 MB/s", "11 Oct 15:29", "",
            "11 Oct 15:20", false, "C:\\Users\\me\\Downloads", 3),
    ]
}

fn mock_main(ui: &MainWindow) {
    ui.global::<Theme>().set_mode(shot_theme());
    ui.set_rows(Rc::new(slint::VecModel::from(mock_rows())).into());

    ui.set_queues(Rc::new(slint::VecModel::from(vec![
        QueueRow { id: 1, name: "Main queue".into(), count: 4, running: true },
        QueueRow { id: 2, name: "Nightly".into(), count: 2, running: false },
    ])).into());

    ui.set_categories(Rc::new(slint::VecModel::from(vec![
        CatRow { name: "video".into(), label: "Video".into(), kind: "video".into(),
                 count: "3".into(), unfinished: "1".into(), finished: "2".into(), builtin: true },
        CatRow { name: "music".into(), label: "Music".into(), kind: "audio".into(),
                 count: "2".into(), unfinished: "".into(), finished: "2".into(), builtin: true },
        CatRow { name: "documents".into(), label: "Documents".into(), kind: "pdf".into(),
                 count: "4".into(), unfinished: "1".into(), finished: "3".into(), builtin: true },
        CatRow { name: "programs".into(), label: "Programs".into(), kind: "exe".into(),
                 count: "2".into(), unfinished: "2".into(), finished: "".into(), builtin: true },
        CatRow { name: "compressed".into(), label: "Compressed".into(), kind: "archive".into(),
                 count: "1".into(), unfinished: "1".into(), finished: "".into(), builtin: true },
        CatRow { name: "images".into(), label: "Images".into(), kind: "image".into(),
                 count: "2".into(), unfinished: "".into(), finished: "2".into(), builtin: true },
    ])).into());

    ui.set_columns(Rc::new(slint::VecModel::from(vec![
        ColDef { key: "name".into(), width: 232.0 },
        ColDef { key: "q".into(), width: 30.0 },
        ColDef { key: "size".into(), width: 78.0 },
        ColDef { key: "status".into(), width: 102.0 },
        ColDef { key: "progress".into(), width: 148.0 },
        ColDef { key: "eta".into(), width: 78.0 },
        ColDef { key: "speed".into(), width: 88.0 },
        ColDef { key: "last".into(), width: 108.0 },
        ColDef { key: "desc".into(), width: 132.0 },
    ])).into());

    ui.set_count_all("12".into());
    ui.set_count_unfinished("5".into());
    ui.set_count_finished("7".into());
    ui.set_selected_id(2);
    ui.set_selected_state(1);
    ui.set_sel_count(1);
    ui.set_can_resume(true);
    ui.set_can_stop(true);
    ui.set_view("all".into());
    ui.set_current_queue(1);
    ui.set_status_text("Downloading 2 of 12 · 19.6 MB/s · 3 queued".into());
    ui.set_limiter_text("Speed limit: 500 KB/s".into());
    ui.set_limiter_on(true);
}

// ---------------------------------------------------------------- rendering

fn arg(n: usize) -> Option<String> {
    std::env::args().nth(n)
}

fn shot_theme() -> i32 {
    arg(3).and_then(|a| a.parse().ok()).unwrap_or(0)
}

fn size_args() -> (usize, usize) {
    (arg(4).and_then(|a| a.parse().ok()).unwrap_or(1180),
     arg(5).and_then(|a| a.parse().ok()).unwrap_or(640))
}

fn save_png(path: &str, w: usize, h: usize, rgb: &[u8]) -> std::io::Result<()> {
    // filter-0 rows
    let mut raw = Vec::with_capacity(h * (w * 3 + 1));
    for y in 0..h {
        raw.push(0u8);
        raw.extend_from_slice(&rgb[y * w * 3..(y + 1) * w * 3]);
    }
    // zlib stream with stored (uncompressed) deflate blocks
    let mut zlib = vec![0x78u8, 0x01];
    let mut chunks = raw.chunks(65535).peekable();
    while let Some(c) = chunks.next() {
        let last = chunks.peek().is_none();
        zlib.push(if last { 1 } else { 0 });
        let n = c.len() as u16;
        zlib.extend_from_slice(&n.to_le_bytes());
        zlib.extend_from_slice(&(!n).to_le_bytes());
        zlib.extend_from_slice(c);
    }
    let (mut a1, mut a2) = (1u32, 0u32);
    for b in &raw {
        a1 = (a1 + *b as u32) % 65521;
        a2 = (a2 + a1) % 65521;
    }
    zlib.extend_from_slice(&((a2 << 16) | a1).to_be_bytes());

    let mut png = Vec::new();
    png.extend_from_slice(&[0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a]);
    let mut ihdr = Vec::new();
    ihdr.extend_from_slice(&(w as u32).to_be_bytes());
    ihdr.extend_from_slice(&(h as u32).to_be_bytes());
    ihdr.extend_from_slice(&[8, 2, 0, 0, 0]);
    chunk(&mut png, b"IHDR", ihdr);
    chunk(&mut png, b"IDAT", zlib);
    chunk(&mut png, b"IEND", vec![]);
    std::fs::write(path, png)
}

fn chunk(out: &mut Vec<u8>, kind: &[u8; 4], data: Vec<u8>) {
    out.extend_from_slice(&(data.len() as u32).to_be_bytes());
    out.extend_from_slice(kind);
    out.extend_from_slice(&data);
    let mut crc = 0xffff_ffffu32;
    for b in kind.iter().chain(data.iter()) {
        crc ^= *b as u32;
        for _ in 0..8 {
            crc = if crc & 1 != 0 { (crc >> 1) ^ 0xedb8_8320 } else { crc >> 1 };
        }
    }
    out.extend_from_slice(&(crc ^ 0xffff_ffff).to_be_bytes());
}

#[derive(Clone, Copy)]
struct Rgb {
    r: u8, g: u8, b: u8,
}

impl TargetPixel for Rgb {
    fn blend(&mut self, color: slint::platform::software_renderer::PremultipliedRgbaColor) {
        if color.alpha == 255 {
            self.r = color.red; self.g = color.green; self.b = color.blue;
        } else if color.alpha > 0 {
            let a = color.alpha as u32;
            let mix = |bg: u8, fg: u8| -> u8 {
                ((fg as u32 * a + bg as u32 * (255 - a)) / 255) as u8
            };
            self.r = mix(self.r, color.red);
            self.g = mix(self.g, color.green);
            self.b = mix(self.b, color.blue);
        }
    }
    fn from_rgb(r: u8, g: u8, b: u8) -> Self {
        Rgb { r, g, b }
    }
    fn background() -> Self {
        Rgb { r: 0, g: 0, b: 0 }
    }
}

fn render_to_png(win: &Rc<MinimalSoftwareWindow>, out: &str) -> anyhow::Result<()> {
    let (w, h) = size_args();
    // the adapter has no window manager: give it an explicit size
    win.set_size(slint::PhysicalSize::new(w as u32, h as u32));
    win.request_redraw();
    let mut saved = false;
    win.draw_if_needed(|renderer| {
        let mut buf = vec![Rgb::background(); w * h];
        renderer.render(&mut buf, w);
        let mut bytes = Vec::with_capacity(w * h * 3);
        for px in &buf {
            bytes.push(px.r); bytes.push(px.g); bytes.push(px.b);
        }
        match save_png(out, w, h, &bytes) {
            Ok(()) => {
                println!("saved {out} ({}x{})", w, h);
                saved = true;
            }
            Err(e) => eprintln!("save_png failed: {e}"),
        }
    });
    if !saved {
        anyhow::bail!("window had nothing to draw (needs_redraw was false)");
    }
    Ok(())
}

fn main() -> anyhow::Result<()> {
    let which = arg(1).unwrap_or_else(|| "main".into());
    let out = arg(2).unwrap_or_else(|| "dios-shot.png".into());

    let win = MinimalSoftwareWindow::new(RepaintBufferType::NewBuffer);
    slint::platform::set_platform(Box::new(ShotPlatform { win: win.clone() }))
        .map_err(|e| anyhow::anyhow!("set_platform: {e}"))?;

    match which.as_str() {
        "add" => {
            let d = AddDialog::new()?;
            d.global::<Theme>().set_mode(shot_theme());
            d.set_url("https://example.com/files/unity-editor-6000.2.1f1.tar.gz".into());
            d.set_save_as("C:\\Users\\me\\Downloads\\unity-editor-6000.2.1f1.tar.gz".into());
            d.set_size_text("1.26 GB".into());
            d.set_kind("archive".into());
            d.set_ext("TAR.GZ".into());
            d.set_info("Direct HTTP link · resumable · 32 connections".into());
            d.set_description("game engine installer".into());
            d.set_category_names(Rc::new(slint::VecModel::from(vec![
                "General".into(), "Compressed".into(), "Documents".into(), "Music".into(),
                "Programs".into(), "Video".into(), "Images".into(),
            ])).into());
            d.set_queue_names(Rc::new(slint::VecModel::from(vec!["Main queue".into(), "Nightly".into()])).into());
            d.show()?;
            render_to_png(&win, &out)?;
        }
        "details" => {
            let d = DetailsDialog::new()?;
            d.global::<Theme>().set_mode(shot_theme());
            d.set_name("ubuntu-24.04.3-desktop-amd64.iso".into());
            d.set_percent("44%".into());
            d.set_url("https://releases.ubuntu.com/24.04/ubuntu-24.04.3-desktop-amd64.iso".into());
            d.set_status("Downloading".into());
            d.set_size("5.8 GB".into());
            d.set_downloaded("2.57 GB of 5.8 GB".into());
            d.set_speed("11.4 MB/s".into());
            d.set_eta("00:07:12".into());
            d.set_resumable("Yes".into());
            d.set_resumable_ok(true);
            d.set_kind("disc".into());
            d.set_progress(0.443);
            d.set_state(1);
            d.set_segs(Rc::new(slint::VecModel::from(vec![
                SegView { busy: true, start: 0.0, done: 0.18, number: "1".into(), downloaded: "1.04 GB".into(), info: "208.1 KB/s".into() },
                SegView { busy: true, start: 0.18, done: 0.33, number: "2".into(), downloaded: "876 MB".into(), info: "191.3 KB/s".into() },
                SegView { busy: false, start: 0.33, done: 0.33, number: "3".into(), downloaded: "0 B".into(), info: "—".into() },
                SegView { busy: true, start: 0.33, done: 0.51, number: "4".into(), downloaded: "1.19 GB".into(), info: "204.9 KB/s".into() },
                SegView { busy: true, start: 0.51, done: 0.66, number: "5".into(), downloaded: "876 MB".into(), info: "190.2 KB/s".into() },
                SegView { busy: false, start: 0.66, done: 0.66, number: "6".into(), downloaded: "0 B".into(), info: "—".into() },
            ])).into());
            d.show()?;
            render_to_png(&win, &out)?;
        }
        _ => {
            let ui = MainWindow::new()?;
            mock_main(&ui);
            ui.show()?;
            render_to_png(&win, &out)?;
        }
    }
    Ok(())
}
