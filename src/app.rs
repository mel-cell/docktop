use crate::docker::{Container, ContainerStats};

pub struct App {
    pub containers: Vec<Container>,
    pub selected_index: usize,
    pub current_stats: Option<ContainerStats>,
    pub should_quit: bool,
}

impl App {
    pub fn new() -> Self {
        Self {
            containers: Vec::new(),
            selected_index: 0,
            current_stats: None,
            should_quit: false,
        }
    }

    pub fn next(&mut self) {
        if !self.containers.is_empty() {
            self.selected_index = (self.selected_index + 1) % self.containers.len();
        }
    }

    pub fn previous(&mut self) {
        if !self.containers.is_empty() {
            if self.selected_index > 0 {
                self.selected_index -= 1;
            } else {
                self.selected_index = self.containers.len() - 1;
            }
        }
    }

    pub fn get_selected_container(&self) -> Option<&Container> {
        self.containers.get(self.selected_index)
    }
}
