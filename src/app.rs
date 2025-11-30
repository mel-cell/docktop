use crate::docker::{Container, ContainerStats, ContainerInspection};
use std::collections::VecDeque;

pub struct App {
    pub containers: Vec<Container>,
    pub selected_index: usize,
    pub current_stats: Option<ContainerStats>,
    pub previous_stats: Option<ContainerStats>, // Added for client-side delta calc
    pub current_inspection: Option<ContainerInspection>,
    pub logs: VecDeque<String>,
    pub is_loading_details: bool,
    pub action_status: Option<(String, std::time::Instant)>,
}

impl App {
    pub fn new() -> Self {
        Self {
            containers: Vec::new(),
            selected_index: 0,
            current_stats: None,
            previous_stats: None,
            current_inspection: None,
            logs: VecDeque::with_capacity(100),
            is_loading_details: false,
            action_status: None,
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
        self.previous_stats = None;
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

    pub fn set_action_status(&mut self, msg: String) {
        self.action_status = Some((msg, std::time::Instant::now()));
    }

    pub fn clear_action_status(&mut self) {
        if let Some((_, time)) = self.action_status {
            if time.elapsed() > std::time::Duration::from_secs(3) {
                self.action_status = None;
            }
        }
    }
}
