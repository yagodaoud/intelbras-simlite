use std::io::Read;
use std::process::{Child, ChildStdout, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use crate::domain::Channel;
use crate::intelbras::RtspUrl;
use crate::security::redact_secrets_in_text;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RgbFrame {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<u8>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DecodeOpts {
    pub width: u32,
    pub height: u32,
    pub fps: u32,
    pub hwaccel: bool,
    pub low_latency: bool,
    pub speed: f32,
}

impl DecodeOpts {
    pub fn mosaic() -> Self {
        Self {
            width: 320,
            height: 180,
            fps: 5,
            hwaccel: true,
            low_latency: true,
            speed: 1.0,
        }
    }

    pub fn focused() -> Self {
        Self {
            width: 640,
            height: 360,
            fps: 8,
            hwaccel: true,
            low_latency: true,
            speed: 1.0,
        }
    }

    pub fn playback(speed: f32) -> Self {
        Self {
            width: 640,
            height: 360,
            fps: 10,
            hwaccel: true,
            low_latency: false,
            speed: if speed <= 0.0 { 1.0 } else { speed },
        }
    }

    pub fn frame_bytes(self) -> usize {
        self.width as usize * self.height as usize * 3
    }
}

#[derive(Clone, Debug)]
pub enum InputSource {
    Rtsp(String),
    TestSrc { width: u32, height: u32, fps: u32 },
}

pub struct FfmpegPlan {
    pub program: String,
    args_before_input: Vec<String>,
    input: String,
    args_after_input: Vec<String>,
}

impl FfmpegPlan {
    pub fn live_or_playback(source: &InputSource, opts: DecodeOpts) -> Self {
        let mut before = vec![
            "-hide_banner".into(),
            "-loglevel".into(),
            "error".into(),
            "-nostdin".into(),
            "-nostats".into(),
        ];
        let mut after = Vec::new();
        let input = match source {
            InputSource::Rtsp(url) => {
                if opts.hwaccel {
                    before.extend(["-hwaccel".into(), "d3d11va".into()]);
                }
                if opts.low_latency {
                    before.extend([
                        "-fflags".into(),
                        "nobuffer".into(),
                        "-flags".into(),
                        "low_delay".into(),
                        "-probesize".into(),
                        "32".into(),
                        "-analyzeduration".into(),
                        "0".into(),
                    ]);
                }
                before.extend([
                    "-threads".into(),
                    "1".into(),
                    "-rtsp_transport".into(),
                    "tcp".into(),
                ]);
                let mut vf = format!("scale={}:{}", opts.width, opts.height);
                if (opts.speed - 1.0).abs() > 0.01 {
                    vf = format!("{vf},setpts=PTS/{speed}", speed = opts.speed);
                }
                after.extend([
                    "-an".into(),
                    "-vf".into(),
                    vf,
                    "-r".into(),
                    opts.fps.to_string(),
                    "-pix_fmt".into(),
                    "rgb24".into(),
                    "-f".into(),
                    "rawvideo".into(),
                    "pipe:1".into(),
                ]);
                url.clone()
            }
            InputSource::TestSrc { width, height, fps } => {
                before.extend(["-f".into(), "lavfi".into()]);
                after.extend([
                    "-an".into(),
                    "-frames:v".into(),
                    "30".into(),
                    "-pix_fmt".into(),
                    "rgb24".into(),
                    "-f".into(),
                    "rawvideo".into(),
                    "pipe:1".into(),
                ]);
                format!("testsrc=size={width}x{height}:rate={fps}")
            }
        };
        Self {
            program: "ffmpeg".into(),
            args_before_input: before,
            input,
            args_after_input: after,
        }
    }

    pub fn argv(&self) -> Vec<String> {
        let mut args = self.args_before_input.clone();
        args.push("-i".into());
        args.push(self.input.clone());
        args.extend(self.args_after_input.clone());
        args
    }

    pub fn redacted_debug(&self) -> String {
        let mut args = self.args_before_input.clone();
        args.push("-i".into());
        args.push(redact_secrets_in_text(&self.input));
        args.extend(self.args_after_input.clone());
        format!("{} {}", self.program, args.join(" "))
    }
}

pub struct FfmpegSession {
    child: Child,
    stdout: ChildStdout,
    width: u32,
    height: u32,
    frame_bytes: usize,
}

impl FfmpegSession {
    pub fn spawn(source: &InputSource, opts: DecodeOpts) -> Result<Self, PlayerError> {
        let plan = FfmpegPlan::live_or_playback(source, opts);
        spawn_plan(&plan, opts.width, opts.height)
    }

    pub fn read_frame(&mut self) -> Result<RgbFrame, PlayerError> {
        let mut pixels = vec![0u8; self.frame_bytes];
        self.stdout
            .read_exact(&mut pixels)
            .map_err(|_| PlayerError::Ended)?;
        Ok(RgbFrame {
            width: self.width,
            height: self.height,
            pixels,
        })
    }
}

impl Drop for FfmpegSession {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn spawn_plan(plan: &FfmpegPlan, width: u32, height: u32) -> Result<FfmpegSession, PlayerError> {
    let mut cmd = Command::new(&plan.program);
    cmd.args(plan.argv())
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    let mut child = cmd.spawn().map_err(|_| PlayerError::MissingFfmpeg)?;
    if let Some(mut stderr) = child.stderr.take() {
        thread::spawn(move || {
            let mut buf = String::new();
            let _ = stderr.read_to_string(&mut buf);
            let redacted = redact_secrets_in_text(&buf);
            if !redacted.trim().is_empty() {
                tracing::warn!(msg = %redacted, "ffmpeg");
            }
        });
    }
    let stdout = child.stdout.take().ok_or(PlayerError::Ended)?;
    Ok(FfmpegSession {
        child,
        stdout,
        width,
        height,
        frame_bytes: width as usize * height as usize * 3,
    })
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum PlayerError {
    #[error("ffmpeg não encontrado no PATH")]
    MissingFfmpeg,
    #[error("stream encerrado")]
    Ended,
}

struct SlotWorker {
    stop: Arc<AtomicBool>,
    latest: Arc<Mutex<Option<RgbFrame>>>,
    generation: Arc<std::sync::atomic::AtomicU64>,
    join: Option<JoinHandle<()>>,
}

impl Drop for SlotWorker {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(join) = self.join.take() {
            let _ = join.join();
        }
    }
}

pub struct PlayerHub {
    workers: std::collections::HashMap<Channel, SlotWorker>,
}

impl PlayerHub {
    pub fn new() -> Self {
        Self {
            workers: std::collections::HashMap::new(),
        }
    }

    pub fn start_rtsp(&mut self, channel: Channel, url: &RtspUrl, opts: DecodeOpts) {
        self.stop(channel);
        let source = InputSource::Rtsp(url.expose().to_string());
        self.workers.insert(channel, spawn_worker(source, opts));
    }

    pub fn stop(&mut self, channel: Channel) {
        self.workers.remove(&channel);
    }

    pub fn stop_all(&mut self) {
        self.workers.clear();
    }

    pub fn latest(&self, channel: Channel) -> Option<RgbFrame> {
        self.workers
            .get(&channel)
            .and_then(|w| w.latest.lock().ok()?.clone())
    }

    pub fn generation(&self, channel: Channel) -> u64 {
        self.workers
            .get(&channel)
            .map(|w| w.generation.load(Ordering::Relaxed))
            .unwrap_or(0)
    }
}

impl Default for PlayerHub {
    fn default() -> Self {
        Self::new()
    }
}

fn spawn_worker(source: InputSource, opts: DecodeOpts) -> SlotWorker {
    let stop = Arc::new(AtomicBool::new(false));
    let latest = Arc::new(Mutex::new(None));
    let generation = Arc::new(std::sync::atomic::AtomicU64::new(0));
    let stop_t = stop.clone();
    let latest_t = latest.clone();
    let gen_t = generation.clone();
    let join = thread::spawn(move || {
        #[cfg(windows)]
        windows_set_below_normal_priority();
        let mut attempt_hw = opts.hwaccel;
        loop {
            if stop_t.load(Ordering::SeqCst) {
                break;
            }
            let mut try_opts = opts;
            try_opts.hwaccel = attempt_hw;
            let mut saw_frame = false;
            match FfmpegSession::spawn(&source, try_opts) {
                Ok(mut session) => {
                    loop {
                        if stop_t.load(Ordering::SeqCst) {
                            break;
                        }
                        match session.read_frame() {
                            Ok(frame) => {
                                saw_frame = true;
                                gen_t.fetch_add(1, Ordering::Relaxed);
                                if let Ok(mut guard) = latest_t.lock() {
                                    *guard = Some(frame);
                                }
                            }
                            Err(_) => break,
                        }
                    }
                }
                Err(PlayerError::MissingFfmpeg) => {
                    thread::sleep(Duration::from_secs(2));
                    break;
                }
                Err(_) => break,
            }
            if attempt_hw && !saw_frame {
                attempt_hw = false;
                continue;
            }
            // Sem vídeo (ex.: sem gravação nesse horário): não fica reiniciando em loop.
            break;
        }
    });
    SlotWorker {
        stop,
        latest,
        generation,
        join: Some(join),
    }
}

#[cfg(windows)]
fn windows_set_below_normal_priority() {
    unsafe extern "system" {
        fn GetCurrentThread() -> *mut core::ffi::c_void;
        fn SetThreadPriority(thread: *mut core::ffi::c_void, priority: i32) -> i32;
    }
    // THREAD_PRIORITY_BELOW_NORMAL
    const PRIORITY: i32 = -1;
    unsafe {
        let _ = SetThreadPriority(GetCurrentThread(), PRIORITY);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn live_plan_uses_tcp_hwaccel_and_substream_scale() {
        let url = "rtsp://viewer:hunter2@192.168.0.10:554/cam/realmonitor?channel=1&subtype=1";
        let plan = FfmpegPlan::live_or_playback(&InputSource::Rtsp(url.into()), DecodeOpts::mosaic());
        let argv = plan.argv();
        assert!(argv.windows(2).any(|w| w == ["-rtsp_transport", "tcp"]));
        assert!(argv.windows(2).any(|w| w == ["-hwaccel", "d3d11va"]));
        assert!(argv.windows(2).any(|w| w == ["-vf", "scale=320:180"]));
        assert!(argv.windows(2).any(|w| w == ["-r", "5"]));
        let debug = plan.redacted_debug();
        assert!(!debug.contains("hunter2"));
        assert!(debug.contains("rtsp://***@192.168.0.10:554"));
    }

    #[test]
    fn debug_plan_never_implements_raw_debug_of_password() {
        let url = "rtsp://viewer:hunter2@host/x";
        let plan = FfmpegPlan::live_or_playback(&InputSource::Rtsp(url.into()), DecodeOpts::mosaic());
        assert!(!plan.redacted_debug().contains("hunter2"));
    }

    #[test]
    fn reads_rgb_frames_from_ffmpeg_testsrc() {
        let opts = DecodeOpts {
            width: 64,
            height: 36,
            fps: 5,
            hwaccel: false,
            low_latency: false,
            speed: 1.0,
        };
        let source = InputSource::TestSrc {
            width: 64,
            height: 36,
            fps: 5,
        };
        let mut session = FfmpegSession::spawn(&source, opts).expect("ffmpeg no PATH");
        let frame = session.read_frame().expect("frame");
        assert_eq!(frame.width, 64);
        assert_eq!(frame.height, 36);
        assert_eq!(frame.pixels.len(), 64 * 36 * 3);
        assert!(frame.pixels.iter().any(|b| *b != 0));
    }
}
