use ratatui::widgets::ListState;
use crate::config::Config;
use serde::Deserialize;

#[derive(Clone)]
pub struct WizardState {
    pub step: WizardStep,
}

#[derive(Clone, Debug)]
pub struct JanitorItem {
    pub id: String,
    pub name: String,
    pub kind: JanitorItemKind,
    pub size: u64,
    pub age: String,
    pub selected: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub enum JanitorItemKind {
    Image,
    Volume,
    Container,
}

#[derive(Clone, PartialEq)]
pub enum PortStatus {
    None,
    Available,
    Occupied(String), // Process info
    Invalid,
}

#[derive(Clone, PartialEq)]
pub enum FileBrowserMode {
    Build,
    Compose,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Framework {
    Laravel,
    NextJs,
    NuxtJs,
    Go,
    Django,
    Rails,
    Rust,
    Python,
    Node,
    Java,
    Static,
    Manual,
}

impl Framework {
    pub fn display_name(&self) -> &str {
        match self {
            Framework::Laravel => "Laravel (PHP)",
            Framework::NextJs => "Next.js (Node)",
            Framework::NuxtJs => "Nuxt.js (Node)",
            Framework::Go => "Go",
            Framework::Rust => "Rust",
            Framework::Django => "Django (Python)",
            Framework::Rails => "Rails (Ruby)",
            Framework::Python => "Python (Generic)",
            Framework::Node => "Node.js (Generic)",
            Framework::Java => "Java (Maven/Gradle)",
            Framework::Static => "Static HTML",
            Framework::Manual => "Manual / Custom",
        }
    }

    pub fn default_port(&self) -> &str {
        match self {
            Framework::Laravel => "8000",
            Framework::NextJs => "3000",
            Framework::NuxtJs => "3000",
            Framework::Go => "8080",
            Framework::Rust => "8080",
            Framework::Django => "8000",
            Framework::Rails => "3000",
            Framework::Python => "5000",
            Framework::Node => "3000",
            Framework::Java => "8080",
            Framework::Static => "80",
            Framework::Manual => "80",
        }
    }
}

#[derive(Clone, Debug)]
pub struct TreeItem {
    pub path: std::path::PathBuf,
    pub depth: usize,
    pub is_dir: bool,
    pub expanded: bool,
    pub is_last: bool,
}

#[derive(Debug, Deserialize)]
pub struct ComposeFile {
    pub services: std::collections::HashMap<String, ServiceConfig>,
}

#[derive(Debug, Deserialize)]
pub struct ServiceConfig {
    #[allow(dead_code)]
    pub image: Option<String>,
    #[allow(dead_code)]
    pub build: Option<serde_yaml::Value>,
}

#[derive(Clone)]
pub enum WizardStep {
    ModeSelection { selected_index: usize },
    QuickRunInput {
        image: String,
        name: String,
        ports: String,
        env: String,
        cpu: String,
        memory: String,
        restart: String,
        show_advanced: bool,
        focused_field: usize,
        editing_id: Option<String>,
        port_status: PortStatus,
        profile: ResourceProfile,
    },
    FileBrowser {
        current_path: std::path::PathBuf,
        list_state: ListState,
        items: Vec<TreeItem>,
        mode: FileBrowserMode,
    },
    DockerfileGenerator {
        path: std::path::PathBuf,
        detected_framework: Framework,
        detected_version: String,
        manual_selection_open: bool,
        manual_selected_index: usize,
        port: String,
        editing_port: bool,
        focused_option: usize,
        port_status: PortStatus,
    },
    BuildConf {
        tag: String,
        mount_volume: bool,
        focused_field: usize,
        path: std::path::PathBuf,
    },
    Processing {
        message: String,
        spinner_frame: usize,
    },
    ComposeGenerator {
        path: std::path::PathBuf,
    },
    ComposeServiceSelection {
        path: std::path::PathBuf,
        selected_services: Vec<String>,
        focused_index: usize,
        all_services: Vec<String>,
    },
    ResourceAllocation {
        path: std::path::PathBuf,
        services: Vec<String>,
        all_services: Vec<String>,
        cpu_limit: String,
        mem_limit: String,
        focused_field: usize,
        detected_cpu: usize,
        detected_mem: u64,
        profile: ResourceProfile,
    },
    Preview {
        title: String,
        content: String,
        action: WizardAction,
        previous_step: Box<WizardStep>,
    },
    Janitor {
        items: Vec<JanitorItem>,
        list_state: ListState,
        loading: bool,
    },
    OverwriteConfirm {
        path: std::path::PathBuf,
        detected_framework: Framework,
        detected_version: String,
        port: String,
    },
    Settings {
        focused_field: usize,
        temp_config: Config,
    },
    Error(String),
}

#[derive(Clone)]
pub enum WizardAction {
    Create { image: String, name: String, ports: String, env: String, cpu: String, memory: String, restart: String },
    Build { tag: String, path: std::path::PathBuf, mount: bool },
    ComposeUp { path: std::path::PathBuf, override_path: Option<std::path::PathBuf> },
    Replace { old_id: String, image: String, name: String, ports: String, env: String, cpu: String, memory: String, restart: String },
    ScanJanitor,
    CleanJanitor(Vec<JanitorItem>),
    Close,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ResourceProfile {
    Eco,
    Standard,
    Performance,
    Custom,
}

impl ResourceProfile {
    pub fn display_name(&self) -> &str {
        match self {
            ResourceProfile::Eco => "Eco (0.5 CPU, 512MB)",
            ResourceProfile::Standard => "Standard (1.0 CPU, 1GB)",
            ResourceProfile::Performance => "Performance (2.0 CPU, 4GB)",
            ResourceProfile::Custom => "Custom",
        }
    }
    
    pub fn values(&self) -> (String, String) {
        match self {
            ResourceProfile::Eco => ("0.5".to_string(), "512m".to_string()),
            ResourceProfile::Standard => ("1.0".to_string(), "1g".to_string()),
            ResourceProfile::Performance => ("2.0".to_string(), "4g".to_string()),
            ResourceProfile::Custom => ("".to_string(), "".to_string()),
        }
    }
}
