use crate::domain::Channel;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Camera {
    pub channel: Channel,
    pub name: String,
}

impl Camera {
    pub fn new(channel: Channel, name: impl Into<String>) -> Self {
        let name = name.into();
        Self {
            name: if name.trim().is_empty() {
                format!("Canal {}", channel)
            } else {
                name
            },
            channel,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_name_falls_back_to_channel_label() {
        let cam = Camera::new(Channel::new(3).unwrap(), "  ");
        assert_eq!(cam.name, "Canal 3");
    }
}
