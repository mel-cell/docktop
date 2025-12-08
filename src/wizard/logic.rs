use std::fs;
use crate::wizard::models::{Framework, PortStatus};

pub fn detect_framework(path: &std::path::Path) -> (Framework, String) {
    if let Ok(content) = fs::read_to_string(path.join("composer.json")) {
        if content.contains("laravel/framework") {
            let version = if let Ok(v) = serde_json::from_str::<serde_json::Value>(&content) {
                v["require"]["php"].as_str()
                    .map(|s| s.chars().skip_while(|c| !c.is_numeric()).take_while(|c| c.is_numeric() || *c == '.').collect::<String>())
                    .unwrap_or("8.2".to_string())
            } else { "8.2".to_string() };
            let version = if version.is_empty() { "8.2".to_string() } else { version };
            return (Framework::Laravel, version);
        }
    }
    if let Ok(content) = fs::read_to_string(path.join("package.json")) {
        let json: serde_json::Value = serde_json::from_str(&content).unwrap_or(serde_json::Value::Null);
        let node_version = json["engines"]["node"].as_str()
            .map(|s| s.chars().skip_while(|c| !c.is_numeric()).take_while(|c| c.is_numeric()).collect::<String>())
            .unwrap_or("18".to_string());
        let node_version = if node_version.is_empty() { "18".to_string() } else { node_version };

        if content.contains("\"next\"") {
            return (Framework::NextJs, node_version);
        }
        if content.contains("\"nuxt\"") {
            return (Framework::NuxtJs, node_version);
        }
        return (Framework::Node, node_version);
    }
    if path.join("go.mod").exists() {
        if let Ok(content) = fs::read_to_string(path.join("go.mod")) {
            for line in content.lines() {
                if line.starts_with("go ") {
                    let v = line.trim_start_matches("go ").trim().to_string();
                    return (Framework::Go, v);
                }
            }
        }
        return (Framework::Go, "1.21".to_string());
    }
    if let Ok(content) = fs::read_to_string(path.join("requirements.txt")) {
        if content.contains("django") {
            return (Framework::Django, "3.11".to_string());
        }
        return (Framework::Python, "3.11".to_string());
    }
    if let Ok(content) = fs::read_to_string(path.join("Gemfile")) {
        if content.contains("rails") {
            return (Framework::Rails, "3.2".to_string());
        }
    }
    if path.join("Cargo.toml").exists() {
        return (Framework::Rust, "latest".to_string());
    }
    if path.join("pom.xml").exists() || path.join("build.gradle").exists() {
        return (Framework::Java, "17".to_string());
    }
    if path.join("index.html").exists() {
        return (Framework::Static, "latest".to_string());
    }
    
    (Framework::Manual, "latest".to_string())
}

pub fn check_port(port_input: &str) -> PortStatus {
    if port_input.is_empty() { return PortStatus::None; }
    
    let port_part = if let Some(idx) = port_input.find(':') {
        &port_input[..idx]
    } else {
        port_input
    };

    if let Ok(port) = port_part.parse::<u16>() {
        match std::net::TcpListener::bind(("0.0.0.0", port)) {
            Ok(_) => PortStatus::Available,
            Err(_) => {
                let output = std::process::Command::new("lsof")
                    .arg("-i")
                    .arg(&format!(":{}", port))
                    .arg("-t")
                    .output();
                
                if let Ok(o) = output {
                    if !o.stdout.is_empty() {
                        let pid_str = String::from_utf8_lossy(&o.stdout).trim().to_string();
                        let pid_str = pid_str.lines().next().unwrap_or("");
                        if let Ok(pid) = pid_str.parse::<i32>() {
                            let ps_out = std::process::Command::new("ps")
                                .arg("-p")
                                .arg(pid_str)
                                .arg("-o")
                                .arg("comm=")
                                .output();
                            if let Ok(ps_o) = ps_out {
                                let name = String::from_utf8_lossy(&ps_o.stdout).trim().to_string();
                                return PortStatus::Occupied(format!("{} (PID: {})", name, pid));
                            }
                            return PortStatus::Occupied(format!("PID: {}", pid));
                        }
                    }
                }
                PortStatus::Occupied("Unknown Process".to_string())
            }
        }
    } else {
        PortStatus::Invalid
    }
}

// For Scaffolding (Creating new project from scratch)
pub fn generate_new_compose_content(services: &[String], cpu: &str, mem: &str) -> String {
    let mut content = String::from("version: '3.8'\nservices:\n  app:\n    build: .\n    ports:\n      - \"80:80\"\n    restart: always\n");
    
    // Add resource limits to app
    if !cpu.is_empty() || !mem.is_empty() {
        content.push_str("    deploy:\n      resources:\n        limits:\n");
        if !cpu.is_empty() {
            content.push_str(&format!("          cpus: '{}'\n", cpu));
        }
        if !mem.is_empty() {
            content.push_str(&format!("          memory: {}\n", mem));
        }
    }

    for svc in services {
        match svc.as_str() {
            "MySQL" => {
                content.push_str("\n  mysql:\n    image: mysql:8.0\n    environment:\n      MYSQL_ROOT_PASSWORD: root\n      MYSQL_DATABASE: app_db\n    ports:\n      - \"3306:3306\"\n");
            },
            "PostgreSQL" => {
                content.push_str("\n  postgres:\n    image: postgres:15\n    environment:\n      POSTGRES_USER: user\n      POSTGRES_PASSWORD: password\n      POSTGRES_DB: app_db\n    ports:\n      - \"5432:5432\"\n");
            },
            "Redis" => {
                content.push_str("\n  redis:\n    image: redis:alpine\n    ports:\n      - \"6379:6379\"\n");
                if !cpu.is_empty() {
                        content.push_str("    deploy:\n      resources:\n        limits:\n          cpus: '0.5'\n          memory: 512M\n");
                }
            },
            "Nginx" => {
                content.push_str("\n  nginx:\n    image: nginx:latest\n    ports:\n      - \"8080:80\"\n    depends_on:\n      - app\n");
            },
            _ => {}
        }
    }
    content
}

pub fn generate_new_compose_file(path: &std::path::Path, services: &[String], cpu: &str, mem: &str) -> std::io::Result<()> {
    if !path.exists() {
        std::fs::create_dir_all(path)?;
    }
    let content = generate_new_compose_content(services, cpu, mem);
    std::fs::write(path.join("docker-compose.yml"), content)
}

// For Existing Projects (The Merge Strategy)
pub fn generate_override_content(services: &[String], cpu: &str, mem: &str) -> String {
    let mut content = String::from("version: '3.8'\nservices:\n");
    
    for svc in services {
        content.push_str(&format!("  {}:\n", svc));
        content.push_str("    deploy:\n      resources:\n        limits:\n");
        
        if !cpu.is_empty() {
            content.push_str(&format!("          cpus: '{}'\n", cpu));
        }
        if !mem.is_empty() {
            content.push_str(&format!("          memory: {}\n", mem));
        }
    }
    content
}

pub fn generate_override_file(path: &std::path::Path, services: &[String], cpu: &str, mem: &str) -> std::io::Result<std::path::PathBuf> {
    let content = generate_override_content(services, cpu, mem);
    let override_path = path.parent().unwrap_or(path).join(".docktop-override.yml");
    std::fs::write(&override_path, content)?;
    Ok(override_path)
}

pub fn detect_resources() -> (usize, u64) {
    use sysinfo::System;
    let mut sys = System::new_all();
    sys.refresh_all();
    (sys.cpus().len(), sys.total_memory())
}

pub fn calculate_auto_resources(total_mem: u64, total_cpus: usize) -> (String, String) {
    let available_mem = (total_mem as f64 * 0.8) as u64;
    let app_mem = (available_mem as f64 * 0.4) as u64;
    
    let mem_str = if app_mem > 1024 * 1024 * 1024 {
        format!("{}G", app_mem / (1024 * 1024 * 1024))
    } else {
        format!("{}M", app_mem / (1024 * 1024))
    };

    let cpu_str = format!("{:.1}", (total_cpus as f64 * 0.25).max(0.5));

    (cpu_str, mem_str)
}

pub fn write_dockerfile(path: &std::path::Path, framework: &Framework, version: &str, port: &str) -> std::io::Result<()> {
    let content = match framework {
        Framework::Laravel => format!(r#"# Generated by DockTop for Laravel (PHP {})
FROM php:{}-fpm

RUN apt-get update && apt-get install -y git curl libpng-dev libonig-dev libxml2-dev zip unzip
RUN docker-php-ext-install pdo_mysql mbstring exif pcntl bcmath gd
COPY --from=composer:latest /usr/bin/composer /usr/bin/composer

WORKDIR /var/www
COPY . .
RUN composer install

CMD php artisan serve --host=0.0.0.0 --port={}
EXPOSE {}
"#, version, version, port, port),
        Framework::NextJs => format!(r#"# Generated by DockTop for Next.js (Node {})
FROM node:{}-alpine AS base

FROM base AS deps
WORKDIR /app
COPY package.json package-lock.json* ./
RUN npm ci

FROM base AS builder
WORKDIR /app
COPY --from=deps /app/node_modules ./node_modules
COPY . .
RUN npm run build

FROM base AS runner
WORKDIR /app
ENV NODE_ENV production
COPY --from=builder /app/public ./public
COPY --from=builder /app/.next/standalone ./
COPY --from=builder /app/.next/static ./.next/static

EXPOSE {}
CMD ["node", "server.js"]
"#, version, version, port),
        Framework::NuxtJs => format!(r#"# Generated by DockTop for Nuxt.js (Node {})
FROM node:{}-alpine AS base

WORKDIR /app
COPY package.json package-lock.json* ./
RUN npm ci

COPY . .
RUN npm run build

ENV HOST 0.0.0.0
ENV PORT {}
EXPOSE {}
CMD ["npm", "run", "start"]
"#, version, version, port, port),
        Framework::Node => format!(r#"# Generated by DockTop for Node.js (Node {})
FROM node:{}-alpine

WORKDIR /app
COPY package.json package-lock.json* ./
RUN npm ci

COPY . .

EXPOSE {}
CMD ["npm", "start"]
"#, version, version, port),
        Framework::Python => format!(r#"# Generated by DockTop for Python (Python {})
FROM python:{}-slim

WORKDIR /app
COPY requirements.txt .
RUN pip install --no-cache-dir -r requirements.txt

COPY . .

EXPOSE {}
CMD ["python", "app.py"]
"#, version, version, port),
        Framework::Django => format!(r#"# Generated by DockTop for Django (Python {})
FROM python:{}-slim

WORKDIR /app
COPY requirements.txt .
RUN pip install --no-cache-dir -r requirements.txt

COPY . .

EXPOSE {}
CMD ["python", "manage.py", "runserver", "0.0.0.0:{}"]
"#, version, version, port, port),
        Framework::Go => format!(r#"# Generated by DockTop for Go (Go {})
FROM golang:{}-alpine

WORKDIR /app
COPY go.mod ./
COPY go.sum ./
RUN go mod download

COPY . .
RUN go build -o /main

EXPOSE {}
CMD ["/main"]
"#, version, version, port),
        Framework::Rust => format!(r#"# Generated by DockTop for Rust
FROM rust:{}-alpine as builder
WORKDIR /usr/src/app
COPY . .
RUN cargo install --path .

FROM alpine:latest
COPY --from=builder /usr/local/cargo/bin/app /usr/local/bin/app
EXPOSE {}
CMD ["app"]
"#, version, port),
        Framework::Java => format!(r#"# Generated by DockTop for Java (OpenJDK {})
FROM openjdk:{}-jdk-alpine

WORKDIR /app
COPY . .
RUN ./mvnw package -DskipTests

EXPOSE {}
CMD ["java", "-jar", "target/app.jar"]
"#, version, version, port),
        Framework::Static => format!(r#"# Generated by DockTop for Static Site
FROM nginx:alpine

COPY . /usr/share/nginx/html

EXPOSE 80
"#),
        _ => format!("FROM alpine\nWORKDIR /app\nCOPY . .\nEXPOSE {}\nCMD [\"/app/main\"]", port),
    };
    
    fs::write(path.join("Dockerfile"), content)?;
    Ok(())
}
