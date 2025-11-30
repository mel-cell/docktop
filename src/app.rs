use crate::docker::{Container, ContainerStats, ContainerInspection};
use std::collections::VecDeque;

pub struct App {
    pub containers: Vec<Container>,
    pub selected_index: usize,
    pub current_stats: Option<ContainerStats>,
    pub current_inspection: Option<ContainerInspection>,
    pub logs: VecDeque<String>,
    pub is_loading_details: bool,
}

impl App {
    pub fn new() -> Self {
        Self {
            containers: Vec::new(),
            selected_index: 0,
            current_stats: None,
            current_inspection: None,
            logs: VecDeque::with_capacity(100),
            is_loading_details: false,
        }
    }

    pub fn next(&mut self) {
        if !self.containers.is_empty() {
            self.selected_index = (self.selected_index + 1) % self.containers.len();
            self.set_loading();
        }
    }

    pub fn previous(&mut self) {
        if !self.containers.is_empty() {
            if self.selected_index > 0 {
                self.selected_index -= 1;
            } else {
                self.selected_index = self.containers.len() - 1;
            }
            self.set_loading();
        }
    }

    fn set_loading(&mut self) {
        self.current_stats = None;
        self.current_inspection = None;
        self.logs.clear();
        self.is_loading_details = true;
    }

    pub fn get_selected_container(&self) -> Option<&Container> {
        self.containers.get(self.selected_index)
    }

    pub fn add_log(&mut self, log: String) {
        if self.logs.len() >= 100 {
            self.logs.pop_front();
        }
        self.logs.push_back(log);
    }
}
