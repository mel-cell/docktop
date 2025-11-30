use crate::docker::{Container, ContainerStats, ContainerInspection};
use crate::config::Config;
use std::collections::VecDeque;



pub struct App {
    pub containers: Vec<Container>,
    pub selected_index: usize,
    pub current_stats: Option<ContainerStats>,
    pub previous_stats: Option<ContainerStats>,
    pub current_inspection: Option<ContainerInspection>,
    pub logs: VecDeque<String>,
    pub is_loading_details: bool,
    pub action_status: Option<(String, std::time::Instant)>,
    pub cpu_history: Vec<(f64, f64)>,
    pub net_rx_history: Vec<(f64, f64)>,
    pub net_tx_history: Vec<(f64, f64)>,
    pub x_axis_bounds: [f64; 2],
    pub net_axis_bounds: [f64; 2],
    pub config: Config,
    pub fishes: Vec<Fish>,
}

#[derive(Clone)]
pub struct Fish {
    pub x: f64,
    pub y: usize, // Vertical lane (0-4)
    pub direction: f64,
    pub speed: f64,
}

impl App {
    pub fn new() -> Self {
        let mut fishes = Vec::new();
        // Initialize 5 fish
        for i in 0..5 {
            fishes.push(Fish {
                x: (i * 5) as f64,
                y: i % 5,
                direction: if i % 2 == 0 { 1.0 } else { -1.0 },
                speed: 0.2 + (i as f64 * 0.1),
            });
        }

        Self {
            containers: Vec::new(),
            selected_index: 0,
            current_stats: None,
            previous_stats: None,
            current_inspection: None,
            logs: VecDeque::with_capacity(100),
            is_loading_details: false,
            action_status: None,
            cpu_history: vec![],
            net_rx_history: vec![],
            net_tx_history: vec![],
            x_axis_bounds: [0.0, 100.0],
            net_axis_bounds: [0.0, 100.0],
            config: Config::load(),
            fishes,
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
        self.cpu_history.clear();
        self.net_rx_history.clear();
        self.net_tx_history.clear();
        self.x_axis_bounds = [0.0, 100.0];
        self.net_axis_bounds = [0.0, 100.0];
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

    pub fn update_cpu_history(&mut self, cpu_usage: f64) {
        let x = if let Some(last) = self.cpu_history.last() {
            last.0 + 1.0
        } else {
            0.0
        };
        
        self.cpu_history.push((x, cpu_usage));
        
        if self.cpu_history.len() > 100 {
            self.cpu_history.remove(0);
        }

        if x > 100.0 {
            self.x_axis_bounds = [x - 100.0, x];
        } else {
            self.x_axis_bounds = [0.0, 100.0];
        }
    }

    pub fn update_net_history(&mut self, rx: f64, tx: f64) {
        let x = if let Some(last) = self.net_rx_history.last() {
            last.0 + 1.0
        } else {
            0.0
        };

        self.net_rx_history.push((x, rx));
        self.net_tx_history.push((x, tx));

        if self.net_rx_history.len() > 100 {
            self.net_rx_history.remove(0);
            self.net_tx_history.remove(0);
        }
        

            


        if x > 100.0 {
            self.net_axis_bounds = [x - 100.0, x];
        } else {
            self.net_axis_bounds = [0.0, 100.0];
        }
    }

    pub fn update_fish(&mut self) {
        for fish in &mut self.fishes {
            fish.x += fish.direction * fish.speed;
            if fish.x > 25.0 {
                fish.direction = -1.0;
            } else if fish.x < 0.0 {
                fish.direction = 1.0;
            }
        }
    }
}
