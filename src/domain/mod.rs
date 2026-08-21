//! Domínio puro: mosaico, câmeras, playback. Sem I/O, fácil de testar.

mod camera;
mod channel;
mod device;
mod error;
mod layout;
mod mosaic;
mod playback;
mod sessions;
mod stream;

pub use camera::Camera;
pub use channel::Channel;
pub use device::Device;
pub use error::DomainError;
pub use layout::Layout;
pub use mosaic::{Mosaic, MosaicSlot};
pub use playback::{
    day_range, month_cells, parse_cgi_ts, sanitize_speed, shift_month, CalendarDay, PlaybackQuery,
    PlaybackSession,
};
pub use sessions::{session_diff, SessionDiff};
pub use stream::{stream_for_view, LiveProfile, StreamKind, ViewMode};
