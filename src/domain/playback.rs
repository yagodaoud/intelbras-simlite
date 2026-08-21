use chrono::{Datelike, Duration, NaiveDate, NaiveDateTime, NaiveTime};

use crate::domain::{Channel, DomainError};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlaybackQuery {
    pub channel: Channel,
    pub start: NaiveDateTime,
    pub end: NaiveDateTime,
}

impl PlaybackQuery {
    pub fn new(
        channel: Channel,
        start: NaiveDateTime,
        end: NaiveDateTime,
    ) -> Result<Self, DomainError> {
        if end <= start {
            return Err(DomainError::InvalidPlaybackRange);
        }
        Ok(Self {
            channel,
            start,
            end,
        })
    }

    pub fn rtsp_start(&self) -> String {
        format_rtsp_ts(self.start)
    }

    pub fn rtsp_end(&self) -> String {
        format_rtsp_ts(self.end)
    }

    pub fn cgi_start(&self) -> String {
        format_cgi_ts(self.start)
    }

    pub fn cgi_end(&self) -> String {
        format_cgi_ts(self.end)
    }

    pub fn with_start(&self, start: NaiveDateTime) -> Result<Self, DomainError> {
        Self::new(self.channel, start, self.end)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct PlaybackSession {
    pub channel: Channel,
    pub range_start: NaiveDateTime,
    pub range_end: NaiveDateTime,
    pub position: NaiveDateTime,
    pub speed: f32,
}

impl PlaybackSession {
    pub fn new(channel: Channel, range_start: NaiveDateTime, range_end: NaiveDateTime) -> Result<Self, DomainError> {
        if range_end <= range_start {
            return Err(DomainError::InvalidPlaybackRange);
        }
        Ok(Self {
            channel,
            range_start,
            range_end,
            position: range_start,
            speed: 1.0,
        })
    }

    pub fn query_from_position(&self) -> Result<PlaybackQuery, DomainError> {
        PlaybackQuery::new(self.channel, self.position, self.range_end)
    }

    pub fn progress(&self) -> f32 {
        let total = (self.range_end - self.range_start).num_milliseconds().max(1) as f32;
        let at = (self.position - self.range_start).num_milliseconds().max(0) as f32;
        (at / total).clamp(0.0, 1.0)
    }

    pub fn seek_ratio(&mut self, ratio: f32) -> Result<(), DomainError> {
        let ratio = ratio.clamp(0.0, 0.999);
        let total_ms = (self.range_end - self.range_start).num_milliseconds();
        let offset = Duration::milliseconds((total_ms as f32 * ratio) as i64);
        let pos = self.range_start + offset;
        if pos >= self.range_end {
            return Err(DomainError::InvalidPlaybackRange);
        }
        self.position = pos;
        Ok(())
    }

    pub fn advance(&mut self, wall_elapsed: Duration) {
        let scaled = Duration::milliseconds(
            (wall_elapsed.num_milliseconds() as f32 * self.speed).round() as i64,
        );
        let next = self.position + scaled;
        self.position = if next >= self.range_end {
            self.range_end
        } else {
            next
        };
    }

    pub fn set_speed(&mut self, speed: f32) {
        self.speed = sanitize_speed(speed);
    }

    pub fn finished(&self) -> bool {
        self.position >= self.range_end
    }
}

pub fn sanitize_speed(speed: f32) -> f32 {
    const ALLOWED: [f32; 4] = [0.5, 1.0, 2.0, 4.0];
    ALLOWED
        .into_iter()
        .min_by(|a, b| {
            (a - speed)
                .abs()
                .partial_cmp(&(b - speed).abs())
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .unwrap_or(1.0)
}

pub fn format_rtsp_ts(ts: NaiveDateTime) -> String {
    ts.format("%Y_%m_%d_%H_%M_%S").to_string()
}

pub fn format_cgi_ts(ts: NaiveDateTime) -> String {
    ts.format("%Y-%m-%d %H:%M:%S").to_string()
}

pub fn parse_cgi_ts(s: &str) -> Result<NaiveDateTime, DomainError> {
    NaiveDateTime::parse_from_str(s.trim(), "%Y-%m-%d %H:%M:%S")
        .or_else(|_| NaiveDateTime::parse_from_str(s.trim(), "%Y-%m-%d %H:%M:%S%.f"))
        .map_err(|_| DomainError::InvalidPlaybackRange)
}

pub fn day_range(date: NaiveDate, hour: u32, minute: u32) -> Result<(NaiveDateTime, NaiveDateTime), DomainError> {
    let time = NaiveTime::from_hms_opt(hour.min(23), minute.min(59), 0)
        .ok_or(DomainError::InvalidPlaybackRange)?;
    let start = date.and_time(time);
    let end = date.and_time(NaiveTime::from_hms_opt(23, 59, 59).unwrap());
    if end <= start {
        return Err(DomainError::InvalidPlaybackRange);
    }
    Ok((start, end))
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CalendarDay {
    pub day: u8,
    pub in_month: bool,
    pub selected: bool,
    pub today: bool,
}

pub fn month_cells(year: i32, month: u32, selected: NaiveDate, today: NaiveDate) -> Vec<CalendarDay> {
    let first = NaiveDate::from_ymd_opt(year, month, 1).unwrap_or(today);
    let start_weekday = first.weekday().num_days_from_sunday() as i64; // Sun-first grid
    let mut cells = Vec::with_capacity(42);
    let cursor = first - Duration::days(start_weekday);
    for i in 0..42 {
        let d = cursor + Duration::days(i);
        cells.push(CalendarDay {
            day: d.day() as u8,
            in_month: d.month() == month && d.year() == year,
            selected: d == selected,
            today: d == today,
        });
    }
    cells
}

pub fn shift_month(year: i32, month: u32, delta: i32) -> (i32, u32) {
    let mut m = month as i32 + delta;
    let mut y = year;
    while m < 1 {
        m += 12;
        y -= 1;
    }
    while m > 12 {
        m -= 12;
        y += 1;
    }
    (y, m as u32)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;

    fn dt(y: i32, m: u32, d: u32, h: u32, min: u32, s: u32) -> NaiveDateTime {
        NaiveDate::from_ymd_opt(y, m, d)
            .unwrap()
            .and_hms_opt(h, min, s)
            .unwrap()
    }

    #[test]
    fn rejects_empty_or_inverted_range() {
        let ch = Channel::new(1).unwrap();
        let a = dt(2026, 8, 21, 10, 0, 0);
        let b = dt(2026, 8, 21, 11, 0, 0);
        assert_eq!(
            PlaybackQuery::new(ch, a, a).unwrap_err(),
            DomainError::InvalidPlaybackRange
        );
        assert_eq!(
            PlaybackQuery::new(ch, b, a).unwrap_err(),
            DomainError::InvalidPlaybackRange
        );
    }

    #[test]
    fn formats_intelbras_rtsp_and_cgi_timestamps() {
        let q = PlaybackQuery::new(
            Channel::new(2).unwrap(),
            dt(2026, 8, 21, 13, 5, 9),
            dt(2026, 8, 21, 14, 0, 0),
        )
        .unwrap();
        assert_eq!(q.rtsp_start(), "2026_08_21_13_05_09");
        assert_eq!(q.cgi_start(), "2026-08-21 13:05:09");
    }

    #[test]
    fn seek_ratio_and_progress() {
        let mut s = PlaybackSession::new(
            Channel::new(1).unwrap(),
            dt(2026, 8, 21, 10, 0, 0),
            dt(2026, 8, 21, 12, 0, 0),
        )
        .unwrap();
        s.seek_ratio(0.5).unwrap();
        assert_eq!(s.position, dt(2026, 8, 21, 11, 0, 0));
        assert!((s.progress() - 0.5).abs() < 0.01);
    }

    #[test]
    fn advance_respects_speed() {
        let mut s = PlaybackSession::new(
            Channel::new(1).unwrap(),
            dt(2026, 8, 21, 10, 0, 0),
            dt(2026, 8, 21, 12, 0, 0),
        )
        .unwrap();
        s.set_speed(2.0);
        s.advance(Duration::seconds(30));
        assert_eq!(s.position, dt(2026, 8, 21, 10, 1, 0));
    }

    #[test]
    fn month_cells_mark_selected_and_today() {
        let selected = NaiveDate::from_ymd_opt(2026, 8, 21).unwrap();
        let today = selected;
        let cells = month_cells(2026, 8, selected, today);
        assert_eq!(cells.len(), 42);
        assert!(cells.iter().any(|c| c.selected && c.day == 21 && c.in_month));
    }
}
