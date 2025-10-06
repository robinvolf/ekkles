use ekkles_data::Song;
use iced::{Element, Task};

use crate::Ekkles;

#[derive(Debug, Clone)]
pub enum Message {}

#[derive(Debug)]
struct Editor {
    song: Option<Song>,
}

impl Editor {
    pub fn new() -> Self {
        Self { song: None }
    }

    pub fn update(state: &mut Ekkles, msg: Message) -> Task<crate::Message> {
        todo!()
    }

    pub fn view(&self) -> Element<Message> {
        todo!()
    }
}
