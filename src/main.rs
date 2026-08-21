slint::include_modules!();

use std::sync::{Arc, Mutex};
use std::time::Instant;

use chrono::{Datelike, Local, NaiveDate, NaiveDateTime};
use simlite::config::{self, AppConfig, CameraFile};
use simlite::domain::{
    day_range, month_cells, shift_month, stream_for_view, Channel, Device, Layout, Mosaic,
    PlaybackSession, SessionDiff, ViewMode,
};
use simlite::intelbras;
use simlite::player::{DecodeOpts, PlayerHub, RgbFrame};
use simlite::security::{CredentialStore, KeyringStore, SecretString};
use slint::{Image, ModelRc, Rgb8Pixel, SharedPixelBuffer, VecModel};

struct State {
    config: AppConfig,
    mosaic: Mosaic,
    hub: PlayerHub,
    secret: Option<SecretString>,
    mode: ViewMode,
    path: std::path::PathBuf,
    live_selected: Vec<Channel>,
    playback: Option<PlaybackSession>,
    playback_tick: Option<Instant>,
    playback_waiting: bool,
    last_ui_gen: std::collections::HashMap<u8, u64>,
    cal_year: i32,
    cal_month: u32,
    cal_selected: NaiveDate,
}

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("warn")),
        )
        .init();

    let ui = AppWindow::new().expect("ui");
    let store = KeyringStore::new();
    let path = config::config_path().unwrap_or_else(|_| std::env::temp_dir().join("simlite-config.json"));

    let today = Local::now().date_naive();
    let (config, mosaic, secret, need_setup) = match config::load(&path) {
        Ok(mut cfg) => {
            // Normaliza host (ex.: "ip:554") e grava de volta — dispositivo persiste entre restarts.
            if let Ok(dev) = cfg.device() {
                cfg.device.host = dev.host.clone();
                cfg.device.rtsp_port = dev.rtsp_port;
                cfg.device.http_port = dev.http_port;
                cfg.device.username = dev.username.clone();
                cfg.device.channel_count = dev.channel_count;
                cfg.device.name = dev.name.clone();
                let _ = config::save(&path, &cfg);
            }
            let mosaic = cfg
                .mosaic()
                .unwrap_or_else(|_| Mosaic::new(Layout::One, default_cameras(6)));
            let secret = store.load(&cfg.device.id).ok();
            let need = secret.is_none();
            (cfg, mosaic, secret, need)
        }
        Err(_) => {
            let cfg = AppConfig::default_six("192.168.0.10", "viewer").expect("default config");
            let mosaic = cfg.mosaic().expect("default mosaic");
            (cfg, mosaic, None, true)
        }
    };

    ui.set_show_setup(need_setup);
    ui.set_setup_host(config.device.host.clone().into());
    ui.set_setup_user(config.device.username.clone().into());
    ui.set_setup_rtsp(config.device.rtsp_port.to_string().into());
    ui.set_setup_http(config.device.http_port.to_string().into());
    ui.set_setup_channels(config.device.channel_count.to_string().into());
    ui.set_device_name(config.device.name.clone().into());
    ui.set_layout_n(layout_n(mosaic.layout()));
    let side = config.mosaic.sidebar_width.clamp(120, 280);
    ui.set_side_width_px(side as f32);
    ui.set_playback_channel(
        mosaic
            .selected()
            .first()
            .or_else(|| mosaic.cameras().first().map(|c| &c.channel))
            .map(|c| i32::from(c.get()))
            .unwrap_or(1),
    );

    let live_selected = mosaic.selected().to_vec();
    let state = Arc::new(Mutex::new(State {
        config,
        mosaic,
        hub: PlayerHub::new(),
        secret,
        mode: ViewMode::Live,
        path,
        live_selected,
        playback: None,
        playback_tick: None,
        playback_waiting: false,
        last_ui_gen: std::collections::HashMap::new(),
        cal_year: today.year(),
        cal_month: today.month(),
        cal_selected: today,
    }));

    {
        let mut st = state.lock().expect("state");
        refresh_cameras(&ui, &st.mosaic);
        refresh_calendar(&ui, &st);
        refresh_slots(&ui, &mut st);
        if st.secret.is_some() {
            restart_live(&mut st);
            ui.set_status("Ao vivo · stream extra no mosaico".into());
            ui.set_live_mode(true);
        }
    }

    let ui_weak = ui.as_weak();
    ui.on_poll_frames({
        let state = state.clone();
        move || {
            if let Some(ui) = ui_weak.upgrade()
                && let Ok(mut st) = state.lock()
            {
                if st.mode == ViewMode::Playback {
                    advance_playback(&mut st, &ui);
                    refresh_playback_ui(&ui, &mut st);
                } else {
                    refresh_slots(&ui, &mut st);
                }
            }
        }
    });

    ui.on_sidebar_resized({
        let state = state.clone();
        move |width| {
            let Ok(mut st) = state.lock() else { return };
            st.config.mosaic.sidebar_width = (width as u32).clamp(120, 280);
            let _ = config::save(&st.path, &st.config);
        }
    });

    ui.on_toggle_camera({
        let state = state.clone();
        let ui_weak = ui.as_weak();
        move |channel| {
            let Some(ui) = ui_weak.upgrade() else { return };
            let Ok(mut st) = state.lock() else { return };
            let Ok(ch) = Channel::new(channel as u8) else { return };
            let Ok(diff) = st.mosaic.toggle(ch) else { return };
            let mosaic = st.mosaic.clone();
            st.config.sync_from_mosaic(&mosaic);
            let _ = config::save(&st.path, &st.config);
            ui.set_layout_n(layout_n(st.mosaic.layout()));
            if st.mode == ViewMode::Live {
                st.live_selected = st.mosaic.selected().to_vec();
                apply_diff(&mut st, diff);
            }
            refresh_cameras(&ui, &st.mosaic);
            ui.set_status(format!("{} câmera(s) · grid automático", st.mosaic.selected().len()).into());
        }
    });

    ui.on_save_setup({
        let state = state.clone();
        let ui_weak = ui.as_weak();
        let store = KeyringStore::new();
        move || {
            let Some(ui) = ui_weak.upgrade() else { return };
            let host = ui.get_setup_host().to_string();
            let user = ui.get_setup_user().to_string();
            let password = ui.get_setup_password().to_string();
            let rtsp = ui.get_setup_rtsp().parse().unwrap_or(554);
            let http = ui.get_setup_http().parse().unwrap_or(80);
            let channels: u8 = ui.get_setup_channels().parse().unwrap_or(6);

            let Ok(mut st) = state.lock() else { return };
            match Device::new(
                st.config.device.id.clone(),
                "DVR",
                host,
                rtsp,
                http,
                user,
                channels.clamp(1, 16),
            ) {
                Ok(device) => {
                    rebuild_cameras(&mut st.config, device.channel_count);
                    st.config.device.id = device.id.clone();
                    st.config.device.name = device.name.clone();
                    st.config.device.host = device.host.clone();
                    st.config.device.rtsp_port = device.rtsp_port;
                    st.config.device.http_port = device.http_port;
                    st.config.device.username = device.username.clone();
                    st.config.device.channel_count = device.channel_count;
                    if !password.is_empty() {
                        let secret = SecretString::new(password);
                        if store.save(&device.id, &secret).is_err() {
                            ui.set_status("Não deu pra guardar a senha no Credential Manager".into());
                            return;
                        }
                        st.secret = Some(secret);
                    }
                    ui.set_setup_password("".into());
                    match st.config.mosaic() {
                        Ok(m) => st.mosaic = m,
                        Err(err) => {
                            ui.set_status(err.to_string().into());
                            return;
                        }
                    }
                    if let Err(err) = config::save(&st.path, &st.config) {
                        ui.set_status(err.to_string().into());
                        return;
                    }
                    restart_live(&mut st);
                    st.mode = ViewMode::Live;
                    ui.set_show_setup(false);
                    ui.set_live_mode(true);
                    ui.set_device_name(st.config.device.name.clone().into());
                    ui.set_layout_n(layout_n(st.mosaic.layout()));
                    refresh_cameras(&ui, &st.mosaic);
                    ui.set_status("Conectado · mosaico ao vivo".into());
                }
                Err(err) => ui.set_status(err.to_string().into()),
            }
        }
    });

    ui.on_go_live({
        let state = state.clone();
        let ui_weak = ui.as_weak();
        move || {
            let Some(ui) = ui_weak.upgrade() else { return };
            let Ok(mut st) = state.lock() else { return };
            let selected = st.live_selected.clone();
            st.mode = ViewMode::Live;
            st.playback = None;
            st.playback_tick = None;
            st.playback_waiting = false;
            let _ = st.mosaic.select_only(&selected);
            restart_live(&mut st);
            ui.set_live_mode(true);
            ui.set_layout_n(layout_n(st.mosaic.layout()));
            refresh_cameras(&ui, &st.mosaic);
            ui.set_status("Ao vivo".into());
        }
    });

    ui.on_go_playback_tab({
        let state = state.clone();
        let ui_weak = ui.as_weak();
        move || {
            let Some(ui) = ui_weak.upgrade() else { return };
            let Ok(mut st) = state.lock() else { return };
            if st.mode == ViewMode::Live {
                st.live_selected = st.mosaic.selected().to_vec();
            }
            st.mode = ViewMode::Playback;
            st.hub.stop_all();
            ui.set_live_mode(false);
            refresh_calendar(&ui, &st);
            ui.set_status("Escolha data/hora e abra a reprodução".into());
        }
    });

    ui.on_calendar_prev({
        let state = state.clone();
        let ui_weak = ui.as_weak();
        move || {
            let Some(ui) = ui_weak.upgrade() else { return };
            let Ok(mut st) = state.lock() else { return };
            let (y, m) = shift_month(st.cal_year, st.cal_month, -1);
            st.cal_year = y;
            st.cal_month = m;
            refresh_calendar(&ui, &st);
        }
    });

    ui.on_calendar_next({
        let state = state.clone();
        let ui_weak = ui.as_weak();
        move || {
            let Some(ui) = ui_weak.upgrade() else { return };
            let Ok(mut st) = state.lock() else { return };
            let (y, m) = shift_month(st.cal_year, st.cal_month, 1);
            st.cal_year = y;
            st.cal_month = m;
            refresh_calendar(&ui, &st);
        }
    });

    ui.on_calendar_pick({
        let state = state.clone();
        let ui_weak = ui.as_weak();
        move |day| {
            let Some(ui) = ui_weak.upgrade() else { return };
            let Ok(mut st) = state.lock() else { return };
            if let Some(date) = NaiveDate::from_ymd_opt(st.cal_year, st.cal_month, day as u32) {
                st.cal_selected = date;
                refresh_calendar(&ui, &st);
            }
        }
    });

    ui.on_start_playback({
        let state = state.clone();
        let ui_weak = ui.as_weak();
        move || {
            let Some(ui) = ui_weak.upgrade() else { return };
            let channel = ui.get_playback_channel();
            let Ok(mut st) = state.lock() else { return };
            let Ok(ch) = Channel::new(channel as u8) else {
                ui.set_status("Canal inválido".into());
                return;
            };
            let Ok((start, end)) = day_range(st.cal_selected, 0, 0) else {
                ui.set_status("Data inválida".into());
                return;
            };
            let Ok(session) = PlaybackSession::new(ch, start, end) else {
                ui.set_status("Intervalo inválido".into());
                return;
            };
            if st.mode == ViewMode::Live {
                st.live_selected = st.mosaic.selected().to_vec();
            }
            st.mode = ViewMode::Playback;
            st.playback = Some(session);
            st.playback_tick = Some(Instant::now());
            st.playback_waiting = true;
            if let Err(err) = start_playback_stream(&mut st) {
                ui.set_status(err.into());
                return;
            }
            ui.set_live_mode(false);
            ui.set_play_has_video(false);
            refresh_playback_ui(&ui, &mut st);
            ui.set_status("Reproduzindo desde 00:00 · se ficar preto, arraste a barra".into());
        }
    });

    ui.on_seek_playback({
        let state = state.clone();
        let ui_weak = ui.as_weak();
        move |ratio| {
            let Some(ui) = ui_weak.upgrade() else { return };
            let Ok(mut st) = state.lock() else { return };
            let Some(session) = st.playback.as_mut() else { return };
            if session.seek_ratio(ratio).is_err() {
                return;
            }
            st.playback_tick = Some(Instant::now());
            st.playback_waiting = true;
            if let Err(err) = start_playback_stream(&mut st) {
                ui.set_status(err.into());
                return;
            }
            ui.set_play_has_video(false);
            refresh_playback_ui(&ui, &mut st);
            ui.set_status("Seek no HD do DVR".into());
        }
    });

    ui.on_set_speed({
        let state = state.clone();
        let ui_weak = ui.as_weak();
        move |speed| {
            let Some(ui) = ui_weak.upgrade() else { return };
            let Ok(mut st) = state.lock() else { return };
            let Some(session) = st.playback.as_mut() else { return };
            session.set_speed(speed);
            let speed_now = session.speed;
            st.playback_tick = Some(Instant::now());
            st.playback_waiting = true;
            ui.set_play_speed(speed_now);
            if let Err(err) = start_playback_stream(&mut st) {
                ui.set_status(err.into());
                return;
            }
            ui.set_play_has_video(false);
            refresh_playback_ui(&ui, &mut st);
        }
    });

    ui.run().expect("run");
}

fn default_cameras(n: u8) -> Vec<simlite::domain::Camera> {
    (1..=n)
        .filter_map(|i| {
            Channel::new(i)
                .ok()
                .map(|ch| simlite::domain::Camera::new(ch, format!("Canal {i}")))
        })
        .collect()
}

fn rebuild_cameras(cfg: &mut AppConfig, count: u8) {
    cfg.cameras = (1..=count)
        .map(|n| CameraFile {
            channel: n,
            name: cfg
                .cameras
                .iter()
                .find(|c| c.channel == n)
                .map(|c| c.name.clone())
                .unwrap_or_else(|| format!("Canal {n}")),
        })
        .collect();
}

fn layout_n(layout: Layout) -> i32 {
    match layout {
        Layout::One => 1,
        Layout::Two => 2,
        Layout::Four => 4,
        Layout::Six => 6,
    }
}

fn decode_opts(layout: Layout, mode: ViewMode, speed: f32) -> DecodeOpts {
    match (mode, layout) {
        (ViewMode::Playback, _) => DecodeOpts::playback(speed),
        (ViewMode::Live, Layout::One) => DecodeOpts::focused(),
        (ViewMode::Live, _) => DecodeOpts::mosaic(),
    }
}

fn restart_live(st: &mut State) {
    st.hub.stop_all();
    st.mode = ViewMode::Live;
    st.playback = None;
    st.playback_tick = None;
    st.playback_waiting = false;
    st.last_ui_gen.clear();
    apply_diff(
        st,
        SessionDiff {
            start: st.mosaic.selected().to_vec(),
            stop: Vec::new(),
        },
    );
}

fn apply_diff(st: &mut State, diff: SessionDiff) {
    for ch in diff.stop {
        st.hub.stop(ch);
        st.last_ui_gen.remove(&ch.get());
    }
    let Some(secret) = st.secret.as_ref() else { return };
    let Ok(device) = st.config.device() else { return };
    let kind = stream_for_view(st.mode, st.mosaic.layout());
    let opts = decode_opts(st.mosaic.layout(), st.mode, 1.0);
    for ch in diff.start {
        if st.mode != ViewMode::Live {
            continue;
        }
        let url = intelbras::live_url(&device, secret, ch, kind);
        st.hub.start_rtsp(ch, &url, opts);
    }
}

fn start_playback_stream(st: &mut State) -> Result<(), &'static str> {
    let secret = st.secret.as_ref().ok_or("Salve a senha do DVR primeiro")?;
    let device = st.config.device().map_err(|_| "Dispositivo inválido")?;
    let session = st.playback.as_ref().ok_or("Sem sessão de reprodução")?;
    let query = session.query_from_position().map_err(|_| "Intervalo inválido")?;
    let speed = session.speed;
    let channel = session.channel;
    let url = intelbras::playback_url(&device, secret, &query);
    st.hub.stop_all();
    st.last_ui_gen.clear();
    st.hub
        .start_rtsp(channel, &url, DecodeOpts::playback(speed));
    Ok(())
}

fn advance_playback(st: &mut State, ui: &AppWindow) {
    let Some(session) = st.playback.as_ref() else { return };
    let channel = session.channel;
    let frame_gen = st.hub.generation(channel);
    if frame_gen == 0 {
        if st.playback_waiting {
            if let Some(started) = st.playback_tick {
                if started.elapsed().as_secs() >= 3 {
                    st.playback_waiting = false;
                    ui.set_status(
                        "Sem vídeo neste horário (pode não haver gravação). Arraste a barra."
                            .into(),
                    );
                }
            }
        }
        return;
    }
    st.playback_waiting = false;
    let Some(tick) = st.playback_tick else { return };
    let elapsed = tick.elapsed();
    if elapsed.as_millis() < 250 {
        return;
    }
    let chrono_elapsed = chrono::Duration::milliseconds(elapsed.as_millis() as i64);
    if let Some(session) = st.playback.as_mut() {
        session.advance(chrono_elapsed);
        if session.finished() {
            st.hub.stop_all();
        }
    }
    st.playback_tick = Some(Instant::now());
}

fn refresh_cameras(ui: &AppWindow, mosaic: &Mosaic) {
    let items: Vec<CameraItem> = mosaic
        .cameras()
        .iter()
        .map(|c| CameraItem {
            channel: i32::from(c.channel.get()),
            name: c.name.clone().into(),
            selected: mosaic.is_selected(c.channel),
        })
        .collect();
    ui.set_cameras(ModelRc::new(VecModel::from(items)));
}

fn refresh_slots(ui: &AppWindow, st: &mut State) {
    let slots = st.mosaic.slots();
    let mut dirty = slots.len() != st.last_ui_gen.len();
    for slot in &slots {
        let frame_gen = st.hub.generation(slot.camera.channel);
        if st.last_ui_gen.get(&slot.camera.channel.get()).copied() != Some(frame_gen) {
            dirty = true;
            break;
        }
    }
    if !dirty {
        ui.set_layout_n(layout_n(st.mosaic.layout()));
        return;
    }
    let items: Vec<SlotView> = slots
        .into_iter()
        .map(|slot| {
            let ch = slot.camera.channel.get();
            let frame_gen = st.hub.generation(slot.camera.channel);
            st.last_ui_gen.insert(ch, frame_gen);
            SlotView {
                channel: i32::from(ch),
                name: slot.camera.name.clone().into(),
                frame: st
                    .hub
                    .latest(slot.camera.channel)
                    .map(|f| frame_to_image(&f))
                    .unwrap_or_default(),
            }
        })
        .collect();
    // Drop gens for cameras no longer shown.
    st.last_ui_gen
        .retain(|ch, _| items.iter().any(|s| s.channel as u8 == *ch));
    ui.set_slots(ModelRc::new(VecModel::from(items)));
    ui.set_layout_n(layout_n(st.mosaic.layout()));
}

fn refresh_calendar(ui: &AppWindow, st: &State) {
    let today = Local::now().date_naive();
    let cells = month_cells(st.cal_year, st.cal_month, st.cal_selected, today);
    let to_cell = |c: &simlite::domain::CalendarDay| DayCell {
        day: i32::from(c.day),
        in_month: c.in_month,
        selected: c.selected,
        today: c.today,
    };
    let mut weeks = Vec::with_capacity(6);
    for chunk in cells.chunks(7) {
        if chunk.len() < 7 {
            break;
        }
        weeks.push(WeekRow {
            d0: to_cell(&chunk[0]),
            d1: to_cell(&chunk[1]),
            d2: to_cell(&chunk[2]),
            d3: to_cell(&chunk[3]),
            d4: to_cell(&chunk[4]),
            d5: to_cell(&chunk[5]),
            d6: to_cell(&chunk[6]),
        });
    }
    ui.set_calendar_weeks(ModelRc::new(VecModel::from(weeks)));
    ui.set_calendar_title(month_title(st.cal_year, st.cal_month).into());
}

fn refresh_playback_ui(ui: &AppWindow, st: &mut State) {
    let Some(session) = st.playback.as_ref() else {
        ui.set_play_progress(0.0);
        ui.set_play_has_video(false);
        return;
    };
    let channel = session.channel;
    let frame_gen = st.hub.generation(channel);
    let has_video = frame_gen > 0;
    ui.set_play_has_video(has_video);
    ui.set_play_progress(session.progress());
    ui.set_play_speed(session.speed);
    ui.set_play_time_label(session.position.format("%H:%M:%S").to_string().into());
    ui.set_play_range_label(
        format!(
            "{} → {}",
            session.range_start.format("%d/%m %H:%M"),
            session.range_end.format("%H:%M")
        )
        .into(),
    );
    ui.set_playback_channel(i32::from(channel.get()));
    let name = st
        .mosaic
        .camera(channel)
        .map(|c| c.name.clone())
        .unwrap_or_else(|| format!("Canal {}", channel));
    ui.set_play_camera_name(name.into());
    let prev = st.last_ui_gen.get(&channel.get()).copied().unwrap_or(0);
    if frame_gen != prev {
        st.last_ui_gen.insert(channel.get(), frame_gen);
        ui.set_play_frame(
            st.hub
                .latest(channel)
                .map(|f| frame_to_image(&f))
                .unwrap_or_default(),
        );
    }
}

fn month_title(year: i32, month: u32) -> String {
    let name = match month {
        1 => "Janeiro",
        2 => "Fevereiro",
        3 => "Março",
        4 => "Abril",
        5 => "Maio",
        6 => "Junho",
        7 => "Julho",
        8 => "Agosto",
        9 => "Setembro",
        10 => "Outubro",
        11 => "Novembro",
        _ => "Dezembro",
    };
    format!("{name} {year}")
}

fn frame_to_image(frame: &RgbFrame) -> Image {
    let mut buf = SharedPixelBuffer::<Rgb8Pixel>::new(frame.width, frame.height);
    let bytes = buf.make_mut_bytes();
    if bytes.len() == frame.pixels.len() {
        bytes.copy_from_slice(&frame.pixels);
    }
    Image::from_rgb8(buf)
}

#[allow(dead_code)]
fn _parse_user_time(s: &str) -> Result<NaiveDateTime, ()> {
    let s = s.trim();
    NaiveDateTime::parse_from_str(s, "%d/%m/%Y %H:%M")
        .or_else(|_| NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S"))
        .map_err(|_| ())
}
