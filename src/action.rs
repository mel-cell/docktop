use crate::wizard::models;
use bollard::Docker;
use bollard::query_parameters::{StartContainerOptions, CreateImageOptions, CreateContainerOptions, StopContainerOptions, RestartContainerOptions, RemoveContainerOptions, ListImagesOptions, ListVolumesOptions, ListContainersOptions, RemoveImageOptions, RemoveVolumeOptions};
use bollard::models::{ContainerCreateBody, HostConfig, PortBinding, RestartPolicy, RestartPolicyNameEnum};
use futures_util::stream::StreamExt;
use tokio::sync::mpsc;

#[derive(Debug, Clone)]
pub enum Action {
    Start(String),
    Stop(String),
    Restart(String),
    Create { 
        image: String, 
        name: String, 
        ports: String, 
        env: String, 
        cpu: String, 
        memory: String,
        restart: String, // Added restart policy
    },
    Build { tag: String, path: std::path::PathBuf, mount: bool },
    ComposeUp { path: std::path::PathBuf, override_path: Option<std::path::PathBuf> },
    Replace { 
        old_id: String, 
        image: String, 
        name: String, 
        ports: String, 
        env: String, 
        cpu: String, 
        memory: String,
        restart: String, // Added restart policy
    },
    ScanJanitor,
    CleanJanitor(Vec<models::JanitorItem>),
    Delete(String),
    RefreshContainers,
}

pub async fn run_action_loop(
    mut rx_action: mpsc::Receiver<Action>,
    tx_action_result: mpsc::Sender<String>,
    tx_janitor_items: mpsc::Sender<Vec<models::JanitorItem>>,
    tx_refresh: mpsc::Sender<()>,
    tx_logs: mpsc::Sender<String>, // Added log channel
) {
    let docker = Docker::connect_with_local_defaults().unwrap();
    
    while let Some(action) = rx_action.recv().await {
        let res = match action {
            Action::RefreshContainers => {
                let _ = tx_refresh.send(()).await;
                "Refreshed containers".to_string()
            },
            Action::ScanJanitor => {
                // ... (existing janitor code)
                let _ = tx_action_result.send("Scanning for junk...".to_string()).await;
                let mut items = Vec::new();
                
                // 1. Dangling Images
                let mut filters = std::collections::HashMap::new();
                filters.insert("dangling".to_string(), vec!["true".to_string()]);
                
                if let Ok(images) = docker.list_images(Some(ListImagesOptions {
                    filters: Some(filters),
                    ..Default::default()
                })).await {
                    for img in images {
                        items.push(models::JanitorItem {
                            id: img.id.clone(),
                            name: "<none>".to_string(),
                            kind: models::JanitorItemKind::Image,
                            size: img.size as u64,
                            age: "Unknown".to_string(),
                            selected: true,
                        });
                    }
                }

                // 2. Unused Volumes
                let mut filters = std::collections::HashMap::new();
                filters.insert("dangling".to_string(), vec!["true".to_string()]);

                if let Ok(volumes) = docker.list_volumes(Some(ListVolumesOptions {
                    filters: Some(filters),
                })).await {
                    if let Some(vols) = volumes.volumes {
                        for vol in vols {
                            items.push(models::JanitorItem {
                                id: vol.name.clone(),
                                name: vol.name.clone(),
                                kind: models::JanitorItemKind::Volume,
                                size: 0,
                                age: "-".to_string(),
                                selected: false,
                            });
                        }
                    }
                }

                // 3. Stopped Containers
                let mut filters = std::collections::HashMap::new();
                filters.insert("status".to_string(), vec!["exited".to_string(), "dead".to_string()]);

                if let Ok(containers) = docker.list_containers(Some(ListContainersOptions {
                    all: true,
                    filters: Some(filters),
                    ..Default::default()
                })).await {
                    for c in containers {
                        items.push(models::JanitorItem {
                            id: c.id.unwrap_or_default(),
                            name: c.names.unwrap_or_default().first().cloned().unwrap_or_default(),
                            kind: models::JanitorItemKind::Container,
                            size: 0, 
                            age: c.status.unwrap_or_default(),
                            selected: true,
                        });
                    }
                }
                
                let _ = tx_janitor_items.send(items).await;
                "Scan Complete".to_string()
            }
            Action::CleanJanitor(items) => {
                 // ... (existing clean code, no changes needed logic-wise, just copy)
                let mut count = 0;
                for item in items {
                    match item.kind {
                        models::JanitorItemKind::Image => {
                            let _ = docker.remove_image(&item.id, None::<RemoveImageOptions>, None).await;
                        },
                        models::JanitorItemKind::Volume => {
                            let _ = docker.remove_volume(&item.id, None::<RemoveVolumeOptions>).await;
                        },
                        models::JanitorItemKind::Container => {
                            let _ = docker.remove_container(&item.id, None::<RemoveContainerOptions>).await;
                        },
                    }
                    count += 1;
                    if count % 5 == 0 {
                            let _ = tx_action_result.send(format!("Cleaned {} items...", count)).await;
                    }
                }
                format!("Janitor finished. Removed {} items.", count)
            }
             Action::Start(id) => {
                match docker.start_container(&id, None::<StartContainerOptions>).await {
                    Ok(_) => format!("Started container {}", &id[..12]),
                    Err(e) => format!("Failed to start: {}", e),
                }
            }
            Action::Stop(id) => {
                match docker.stop_container(&id, None::<StopContainerOptions>).await {
                    Ok(_) => format!("Stopped container {}", &id[..12]),
                    Err(e) => format!("Failed to stop: {}", e),
                }
            }
            Action::Restart(id) => {
                match docker.restart_container(&id, None::<RestartContainerOptions>).await {
                    Ok(_) => format!("Restarted container {}", &id[..12]),
                    Err(e) => format!("Failed to restart: {}", e),
                }
            }
             Action::Create { image, name, ports, env, cpu, memory, restart } => {
                // ... (Copy existing logic)
                let _ = tx_action_result.send(format!("Pulling {}...", image)).await;
                let mut stream = docker.create_image(
                    Some(CreateImageOptions { from_image: Some(image.clone()), ..Default::default() }),
                    None,
                    None
                );
                
                let mut last_status = String::new();
                while let Some(chunk) = stream.next().await {
                    if let Ok(info) = chunk {
                        let status = info.status.unwrap_or_default();
                        let progress = info.progress.unwrap_or_default();
                        let current_msg = format!("{} {}", status, progress);
                        
                        // Simple throttle/dedup
                        if current_msg != last_status {
                            let _ = tx_action_result.send(format!("Pulling {}: {}", image, current_msg)).await;
                            last_status = current_msg;
                        }
                    }
                }

                let _ = tx_action_result.send(format!("Creating {}...", name)).await;
                
                let mut port_bindings = std::collections::HashMap::new();
                let mut exposed_ports = std::collections::HashMap::new();
                if !ports.is_empty() {
                    // Split by space or comma to support multiple ports
                    for port_def in ports.split(|c| c == ' ' || c == ',') {
                        let port_def = port_def.trim();
                        if port_def.is_empty() { continue; }
                        
                        let parts: Vec<&str> = port_def.split(':').collect();
                        let (host_port, container_port_raw) = if parts.len() == 2 {
                            (parts[0].trim().to_string(), parts[1].trim())
                        } else if parts.len() == 1 {
                            let p = parts[0].trim();
                            (p.to_string(), p)
                        } else {
                            continue;
                        };

                        let container_port = format!("{}/tcp", container_port_raw);
                        
                        exposed_ports.insert(container_port.clone(), std::collections::HashMap::new());
                        
                        // Check if entry already exists
                        port_bindings.entry(container_port)
                            .or_insert_with(|| Some(Vec::new()))
                            .as_mut()
                            .map(|v| v.push(PortBinding {
                                host_ip: Some("0.0.0.0".to_string()),
                                host_port: Some(host_port),
                            }));
                    }
                }

                let nano_cpus = if !cpu.is_empty() {
                    cpu.parse::<f64>().ok().map(|v| (v * 1_000_000_000.0) as i64)
                } else { None };

                let memory_bytes = if !memory.is_empty() {
                    let lower = memory.to_lowercase();
                    if let Some(val) = lower.strip_suffix('m') {
                        val.parse::<i64>().ok().map(|v| v * 1024 * 1024)
                    } else if let Some(val) = lower.strip_suffix('g') {
                        val.parse::<i64>().ok().map(|v| v * 1024 * 1024 * 1024)
                    } else if let Some(val) = lower.strip_suffix('k') {
                        val.parse::<i64>().ok().map(|v| v * 1024)
                    } else {
                        lower.parse::<i64>().ok()
                    }
                } else { None };

                // robust env splitting
                let envs = if !env.is_empty() { 
                    Some(env.split(|c| c == ' ' || c == ';')
                        .map(|s| s.trim().to_string())
                        .filter(|s| !s.is_empty())
                        .collect()) 
                } else { None };
                
                let restart_policy = if !restart.is_empty() {
                    let name = match restart.as_str() {
                        "always" => RestartPolicyNameEnum::ALWAYS,
                        "unless-stopped" => RestartPolicyNameEnum::UNLESS_STOPPED,
                        "on-failure" => RestartPolicyNameEnum::ON_FAILURE,
                        _ => RestartPolicyNameEnum::NO,
                    };
                    Some(RestartPolicy { name: Some(name), maximum_retry_count: None })
                } else {
                    None
                };
                
                let config = ContainerCreateBody {
                    image: Some(image.clone()),
                    exposed_ports: Some(exposed_ports),
                    host_config: Some(HostConfig {
                        port_bindings: Some(port_bindings),
                        nano_cpus,
                        memory: memory_bytes,
                        restart_policy,
                        ..Default::default()
                    }),
                    env: envs,
                    ..Default::default()
                };

                let options = if !name.is_empty() {
                    Some(CreateContainerOptions { name: Some(name.clone()), ..Default::default() })
                } else { None };

                match docker.create_container(options, config).await {
                    Ok(res) => {
                        let _ = tx_action_result.send(format!("Starting {}...", res.id)).await;
                        match docker.start_container(&res.id, None::<StartContainerOptions>).await {
                            Ok(_) => format!("Started new container {}", &res.id[..12]),
                            Err(e) => format!("Failed to start: {}", e),
                        }
                    },
                    Err(e) => format!("Failed to create: {}", e),
                }
            }
            Action::Build { tag, path, mount } => {
                    let _ = tx_action_result.send(format!("Building {}...", tag)).await;
                    
                    // Use CLI with pipes to capture output
                    let mut cmd = std::process::Command::new("docker");
                    cmd.arg("build")
                       .arg("-t")
                       .arg(&tag)
                       .current_dir(&path)
                       .arg(".")
                       .stdout(std::process::Stdio::piped())
                       .stderr(std::process::Stdio::piped());

                    if let Ok(mut child) = cmd.spawn() {
                        // Stream Logs
                        if let Some(stdout) = child.stdout.take() {
                             let tx = tx_logs.clone();
                             tokio::spawn(async move {
                                 use std::io::{BufRead, BufReader};
                                 let reader = BufReader::new(stdout);
                                 for line in reader.lines() {
                                     if let Ok(l) = line {
                                         let _ = tx.send(format!("[BUILD] {}", l)).await;
                                     }
                                 }
                             });
                        }
                        if let Some(stderr) = child.stderr.take() {
                             let tx = tx_logs.clone();
                             tokio::spawn(async move {
                                 use std::io::{BufRead, BufReader};
                                 let reader = BufReader::new(stderr);
                                 for line in reader.lines() {
                                     if let Ok(l) = line {
                                         let _ = tx.send(format!("[BUILD ERR] {}", l)).await;
                                     }
                                 }
                             });
                        }

                        let output = child.wait_with_output(); // Wait for completion
                        
                        match output {
                             Ok(o) => {
                                if o.status.success() {
                                    let _ = tx_action_result.send(format!("Build successful for {}", tag)).await;
                                    
                                    // Run
                                    let _ = tx_action_result.send(format!("Running {}...", tag)).await;
                                    let mut run_cmd = std::process::Command::new("docker");
                                    run_cmd.arg("run")
                                           .arg("-d")
                                           .arg("--name")
                                           .arg(format!("docktop_{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs())) 
                                           .arg("-P"); 
                                    
                                    if mount {
                                        if let Ok(abs_path) = std::fs::canonicalize(&path) {
                                            run_cmd.arg("-v").arg(format!("{}:/app", abs_path.to_string_lossy()));
                                        }
                                    }
                                    
                                    run_cmd.arg(&tag);
                                    
                                    match run_cmd.output() {
                                        Ok(run_o) => {
                                            if run_o.status.success() {
                                                let id = String::from_utf8_lossy(&run_o.stdout).trim().to_string();
                                                format!("Built and started {}", &id[..12.min(id.len())])
                                            } else {
                                                format!("Built but failed to run: {}", String::from_utf8_lossy(&run_o.stderr))
                                            }
                                        },
                                        Err(e) => format!("Built but failed to execute run: {}", e)
                                    }
                                } else {
                                    format!("Build Failed. Check Logs.")
                                }
                             }
                             Err(e) => format!("Failed to wait for build: {}", e)
                        }
                    } else {
                        format!("Failed to spawn docker build")
                    }
            }
            Action::ComposeUp { path, override_path } => {
                let _ = tx_action_result.send("Running docker compose up...".to_string()).await;
                
                let (work_dir, main_file) = if path.is_file() {
                    (path.parent().unwrap_or(&path).to_path_buf(), path.file_name().unwrap().to_string_lossy().to_string())
                } else {
                    (path.clone(), "docker-compose.yml".to_string())
                };

                let mut cmd = std::process::Command::new("docker");
                cmd.arg("compose")
                    .arg("-f")
                    .arg(&main_file);
                
                if let Some(ref ovr) = override_path {
                    if let Some(ovr_name) = ovr.file_name() {
                        cmd.arg("-f").arg(ovr_name);
                    }
                }

                cmd.arg("up")
                    .arg("-d")
                    .current_dir(&work_dir);

                let output = cmd.output();
                    
                match output {
                    Ok(o) => {
                        // Cleanup override file
                        if let Some(ovr) = override_path {
                            let _ = std::fs::remove_file(ovr);
                        }

                        if o.status.success() {
                            "Compose Up Successful".to_string()
                        } else {
                            format!("Compose Failed: {}", String::from_utf8_lossy(&o.stderr))
                        }
                    },
                    Err(e) => {
                            // Cleanup override file
                        if let Some(ovr) = override_path {
                            let _ = std::fs::remove_file(ovr);
                        }
                        format!("Failed to run compose: {}", e)
                    },
                }
            }
            Action::Replace { old_id, image, name, ports, env, cpu, memory, restart } => {
                    let _ = tx_action_result.send(format!("Stopping {}...", old_id)).await;
                    let _ = docker.stop_container(&old_id, None::<StopContainerOptions>).await;
                    let _ = tx_action_result.send(format!("Removing {}...", old_id)).await;
                    let _ = docker.remove_container(&old_id, None::<RemoveContainerOptions>).await;
                    
                let _ = tx_action_result.send(format!("Pulling {}...", image)).await;
                let mut stream = docker.create_image(
                    Some(CreateImageOptions { from_image: Some(image.clone()), ..Default::default() }),
                    None,
                    None
                );
                while let Some(_) = stream.next().await {}

                let _ = tx_action_result.send(format!("Creating {}...", name)).await;
                
                let mut port_bindings = std::collections::HashMap::new();
                let mut exposed_ports = std::collections::HashMap::new();
                if !ports.is_empty() {
                        let parts: Vec<&str> = ports.split(':').collect();
                        if parts.len() == 2 {
                            let container_port = format!("{}/tcp", parts[1]);
                            exposed_ports.insert(container_port.clone(), std::collections::HashMap::new());
                            port_bindings.insert(container_port, Some(vec![PortBinding {
                                host_ip: Some("0.0.0.0".to_string()),
                                host_port: Some(parts[0].to_string()),
                            }]));
                        }
                }

                let nano_cpus = if !cpu.is_empty() {
                    cpu.parse::<f64>().ok().map(|v| (v * 1_000_000_000.0) as i64)
                } else { None };

                let memory_bytes = if !memory.is_empty() {
                    let lower = memory.to_lowercase();
                    if let Some(val) = lower.strip_suffix('m') {
                        val.parse::<i64>().ok().map(|v| v * 1024 * 1024)
                    } else if let Some(val) = lower.strip_suffix('g') {
                        val.parse::<i64>().ok().map(|v| v * 1024 * 1024 * 1024)
                    } else if let Some(val) = lower.strip_suffix('k') {
                        val.parse::<i64>().ok().map(|v| v * 1024)
                    } else {
                        lower.parse::<i64>().ok()
                    }
                } else { None };

                let envs = if !env.is_empty() { Some(vec![env]) } else { None };
                
                // Restart Policy
                let restart_policy = if !restart.is_empty() {
                    let name = match restart.as_str() {
                        "always" => RestartPolicyNameEnum::ALWAYS,
                        "unless-stopped" => RestartPolicyNameEnum::UNLESS_STOPPED,
                        "on-failure" => RestartPolicyNameEnum::ON_FAILURE,
                        _ => RestartPolicyNameEnum::NO,
                    };
                    Some(RestartPolicy { name: Some(name), maximum_retry_count: None })
                } else {
                    None
                };
                
                let config = ContainerCreateBody {
                    image: Some(image.clone()),
                    exposed_ports: Some(exposed_ports),
                    host_config: Some(HostConfig {
                        port_bindings: Some(port_bindings),
                        nano_cpus,
                        memory: memory_bytes,
                        restart_policy,
                        ..Default::default()
                    }),
                    env: envs,
                    ..Default::default()
                };

                let options = if !name.is_empty() {
                    Some(CreateContainerOptions { name: Some(name.clone()), ..Default::default() })
                } else { None };

                match docker.create_container(options, config).await {
                    Ok(res) => {
                        let _ = tx_action_result.send(format!("Starting {}...", res.id)).await;
                        match docker.start_container(&res.id, None::<StartContainerOptions>).await {
                            Ok(_) => format!("Replaced container {}", &res.id[..12]),
                            Err(e) => format!("Failed to start: {}", e),
                        }
                    },
                    Err(e) => format!("Failed to create: {}", e),
                }
            }
            Action::Delete(id) => {
                let _ = tx_action_result.send(format!("Removing {}...", id)).await;
                match docker.remove_container(&id, Some(RemoveContainerOptions { force: true, ..Default::default() })).await {
                    Ok(_) => format!("Removed container {}", &id[..12]),
                    Err(e) => format!("Failed to remove: {}", e),
                }
            }
        };
        let _ = tx_action_result.send(res).await;
    }
}
