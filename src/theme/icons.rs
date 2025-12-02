#[allow(dead_code)]
pub struct IconSet;

#[allow(dead_code)]
impl IconSet {
    // UI Icons
    pub const SPINNER: &'static [&'static str] = &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
    pub const CHECK: &'static str = "✔"; // 
    pub const CROSS: &'static str = "✖"; // 
    pub const WARNING: &'static str = "⚠"; // 
    pub const INFO: &'static str = "ℹ"; // 
    
    // File System Icons
    pub const FOLDER_OPEN: &'static str = "ﱮ"; // ﱮ
    pub const FOLDER_CLOSED: &'static str = ""; // 
    pub const FILE_DEFAULT: &'static str = ""; // 
    
    // Docker Icons
    pub const DOCKER: &'static str = "";
    pub const CONTAINER: &'static str = ""; // 
    pub const IMAGE: &'static str = ""; // 
    pub const VOLUME: &'static str = ""; // 
    pub const NETWORK: &'static str = ""; // 
    pub const CPU: &'static str = ""; // 
    pub const MEMORY: &'static str = ""; // 
    
    // Tech Stack Icons
    pub const RUST: &'static str = "";
    pub const GO: &'static str = "";
    pub const PYTHON: &'static str = "";
    pub const JS: &'static str = "";
    pub const TS: &'static str = "";
    pub const PHP: &'static str = "";
    pub const LARAVEL: &'static str = "";
    pub const HTML: &'static str = "";
    pub const CSS: &'static str = "";
    pub const DB: &'static str = "";
    pub const MYSQL: &'static str = "";
    pub const POSTGRES: &'static str = "";
    pub const REDIS: &'static str = "";
    pub const NGINX: &'static str = "";
    pub const APACHE: &'static str = "";
    pub const LINUX: &'static str = "";

    pub fn get_file_icon(filename: &str) -> &'static str {
        let lower = filename.to_lowercase();
        if lower == "dockerfile" { return Self::DOCKER; }
        if lower == "docker-compose.yml" || lower == "docker-compose.yaml" { return Self::DOCKER; }
        if lower == "makefile" { return ""; }
        if lower == "cargo.toml" { return Self::RUST; }
        if lower == "package.json" { return ""; }
        if lower == "go.mod" { return Self::GO; }

        if let Some(ext) = std::path::Path::new(filename).extension().and_then(|e| e.to_str()) {
            match ext.to_lowercase().as_str() {
                "rs" => Self::RUST,
                "go" => Self::GO,
                "py" => Self::PYTHON,
                "js" => Self::JS,
                "ts" | "tsx" => Self::TS,
                "php" => Self::PHP,
                "html" => Self::HTML,
                "css" => Self::CSS,
                "sql" => Self::DB,
                "sh" => "",
                "json" | "yaml" | "yml" | "toml" => "",
                "md" => "",
                "lock" => "",
                _ => Self::FILE_DEFAULT,
            }
        } else {
            Self::FILE_DEFAULT
        }
    }

    pub fn get_container_icon(image: &str) -> &'static str {
        let lower = image.to_lowercase();
        if lower.contains("mysql") || lower.contains("mariadb") { return Self::MYSQL; }
        if lower.contains("postgres") { return Self::POSTGRES; }
        if lower.contains("redis") { return Self::REDIS; }
        if lower.contains("mongo") { return ""; }
        if lower.contains("nginx") { return Self::NGINX; }
        if lower.contains("httpd") || lower.contains("apache") { return Self::APACHE; }
        if lower.contains("node") { return Self::JS; }
        if lower.contains("python") { return Self::PYTHON; }
        if lower.contains("golang") || lower.contains("go") { return Self::GO; }
        if lower.contains("rust") { return Self::RUST; }
        if lower.contains("php") { return Self::PHP; }
        if lower.contains("wordpress") { return ""; }
        if lower.contains("alpine") || lower.contains("ubuntu") || lower.contains("debian") { return Self::LINUX; }
        
        Self::CONTAINER
    }
}
