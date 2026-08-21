//! Dialeto Intelbras/Dahua: RTSP live/playback e CGI de gravações.

mod cgi;
mod digest;
mod kvp;
mod rtsp;

pub use cgi::{find_recordings, DigestCgi, Recording, RecordingFinderError};
pub use rtsp::{live_url, playback_url, RtspUrl};
