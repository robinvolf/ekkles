use ekkles_data::playlist::PlaylistMetadata;

#[derive(Debug)]
pub struct PlaylistEditor {
    playlist: PlaylistMetadata,
}

impl PlaylistEditor {
    pub fn new(playlist: PlaylistMetadata) -> Self {
        Self { playlist }
    }
}
