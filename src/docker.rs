#![allow(dead_code)]
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::UnixStream;
use serde::Deserialize;
use anyhow::Result;
use std::collections::HashMap;

#[derive(Debug, Deserialize, Clone)]
pub struct Container {
    #[serde(rename = "Id")]
    pub id: String,
    #[serde(rename = "Names")]
    pub names: Vec<String>,
    #[serde(rename = "Image")]
    pub image: String,
    #[serde(rename = "State")]
    pub state: String,
    #[serde(rename = "Status")]
    pub status: String,
    #[serde(rename = "Ports")]
    pub ports: Vec<Port>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Port {
    #[serde(rename = "IP")]
    pub ip: Option<String>,
    #[serde(rename = "PrivatePort")]
    pub private_port: u16,
    #[serde(rename = "PublicPort")]
    pub public_port: Option<u16>,
    #[serde(rename = "Type")]
    pub type_: String,
}



#[derive(Debug, Deserialize, Clone)]
pub struct CpuStats {
    pub cpu_usage: CpuUsage,
    pub system_cpu_usage: Option<u64>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct CpuUsage {
    pub total_usage: u64,
    pub percpu_usage: Option<Vec<u64>>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct MemoryStats {
    pub usage: Option<u64>,
    pub limit: Option<u64>,
    pub stats: Option<HashMap<String, u64>>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ContainerStats {
    pub cpu_stats: CpuStats,
    pub precpu_stats: CpuStats,
    pub memory_stats: MemoryStats,
    pub networks: Option<HashMap<String, NetworkStats>>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct NetworkStats {
    #[serde(rename = "rx_bytes")]
    pub rx_bytes: u64,
    #[serde(rename = "tx_bytes")]
    pub tx_bytes: u64,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ContainerInspection {
    #[serde(rename = "Id")]
    pub id: String,
    #[serde(rename = "Created")]
    pub created: Option<String>,
    #[serde(rename = "Path")]
    pub path: Option<String>,
    #[serde(rename = "Args")]
    pub args: Option<Vec<String>>,
    #[serde(rename = "Config")]
    pub config: Option<ContainerConfig>,
    #[serde(rename = "Name")]
    pub name: Option<String>,
    #[serde(rename = "NetworkSettings")]
    pub network_settings: Option<NetworkSettings>,
    #[serde(rename = "HostConfig")]
    pub host_config: Option<HostConfig>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct HostConfig {
    #[serde(rename = "NanoCpus")]
    pub nano_cpus: Option<i64>,
    #[serde(rename = "Memory")]
    pub memory: Option<i64>,
    #[serde(rename = "RestartPolicy")]
    pub restart_policy: Option<RestartPolicy>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct RestartPolicy {
    #[serde(rename = "Name")]
    pub name: String,
    #[serde(rename = "MaximumRetryCount")]
    pub maximum_retry_count: Option<i64>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ContainerConfig {
    #[serde(rename = "Image")]
    pub image: String,
    #[serde(rename = "Cmd")]
    pub cmd: Option<Vec<String>>,
    #[serde(rename = "Env")]
    pub env: Option<Vec<String>>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct NetworkSettings {
    #[serde(rename = "Ports")]
    pub ports: Option<HashMap<String, Option<Vec<PortBinding>>>>,
    #[serde(rename = "IPAddress")]
    pub ip_address: Option<String>,
    #[serde(rename = "Networks")]
    pub networks: Option<HashMap<String, Network>>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Network {
    #[serde(rename = "IPAddress")]
    pub ip_address: Option<String>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct PortBinding {
    #[serde(rename = "HostIp")]
    pub host_ip: String,
    #[serde(rename = "HostPort")]
    pub host_port: String,
}

pub struct DockerClient {
    socket_path: String,
}

impl DockerClient {
    pub fn new() -> Self {
        Self {
            socket_path: "/var/run/docker.sock".to_string(),
        }
    }

    async fn send_request(&self, request: &str) -> Result<String> {
        let mut stream = UnixStream::connect(&self.socket_path).await?;
        stream.write_all(request.as_bytes()).await?;

        let mut response = Vec::new();
        stream.read_to_end(&mut response).await?;

        let response_str = String::from_utf8_lossy(&response);
        
        let parts: Vec<&str> = response_str.splitn(2, "\r\n\r\n").collect();
        if parts.len() < 2 {
            // Check if it's a 204 No Content (common for start/stop/restart)
            if response_str.starts_with("HTTP/1.1 204") {
                return Ok("".to_string());
            }
            // Check for 304 Not Modified
            if response_str.starts_with("HTTP/1.1 304") {
                return Ok("".to_string());
            }
             // Check for 200 OK
             if response_str.starts_with("HTTP/1.1 200") {
                 // Might be empty body?
                 return Ok("".to_string());
             }

            return Err(anyhow::anyhow!("Invalid response from Docker daemon: {}", response_str.chars().take(100).collect::<String>()));
        }
        
        Ok(parts[1].to_string())
    }

    pub async fn list_containers(&self) -> Result<Vec<Container>> {
        let request = "GET /containers/json?all=true HTTP/1.0\r\nHost: localhost\r\nConnection: close\r\n\r\n";
        let body = self.send_request(request).await?;
        let containers: Vec<Container> = serde_json::from_str(&body)?;
        Ok(containers)
    }

    pub async fn get_stats(&self, container_id: &str) -> Result<ContainerStats> {
        let request = format!("GET /containers/{}/stats?stream=false HTTP/1.0\r\nHost: localhost\r\nConnection: close\r\n\r\n", container_id);
        let body = self.send_request(&request).await?;
        let stats: ContainerStats = serde_json::from_str(&body)?;
        Ok(stats)
    }

    pub async fn inspect_container(&self, container_id: &str) -> Result<ContainerInspection> {
        let request = format!("GET /containers/{}/json HTTP/1.0\r\nHost: localhost\r\nConnection: close\r\n\r\n", container_id);
        let body = self.send_request(&request).await?;
        let inspection: ContainerInspection = serde_json::from_str(&body)?;
        Ok(inspection)
    }

    pub async fn get_logs_stream(&self, container_id: &str) -> Result<UnixStream> {
        let mut stream = UnixStream::connect(&self.socket_path).await?;
        let request = format!(
            "GET /containers/{}/logs?stdout=true&stderr=true&tail=100&follow=true HTTP/1.0\r\nHost: localhost\r\nConnection: Upgrade\r\nUpgrade: tcp\r\n\r\n", 
            container_id
        );
        stream.write_all(request.as_bytes()).await?;

        // Consume HTTP headers
        let mut buffer = [0u8; 1];
        let mut headers = Vec::new();
        loop {
            stream.read_exact(&mut buffer).await?;
            headers.push(buffer[0]);
            
            if headers.len() >= 4 {
                if &headers[headers.len()-4..] == b"\r\n\r\n" {
                    break;
                }
            }
        }

        Ok(stream)
    }

    pub async fn get_events_stream(&self) -> Result<UnixStream> {
        let mut stream = UnixStream::connect(&self.socket_path).await?;
        let request = "GET /events?filters=%7B%22type%22%3A%5B%22container%22%5D%7D HTTP/1.0\r\nHost: localhost\r\nConnection: close\r\n\r\n";
        stream.write_all(request.as_bytes()).await?;

        // Consume HTTP headers
        let mut buffer = [0u8; 1];
        let mut headers = Vec::new();
        loop {
            stream.read_exact(&mut buffer).await?;
            headers.push(buffer[0]);
            
            if headers.len() >= 4 {
                if &headers[headers.len()-4..] == b"\r\n\r\n" {
                    break;
                }
            }
        }

        Ok(stream)
    }

    pub async fn start_container(&self, container_id: &str) -> Result<()> {
        let request = format!("POST /containers/{}/start HTTP/1.0\r\nHost: localhost\r\nConnection: close\r\n\r\n", container_id);
        self.send_request(&request).await?;
        Ok(())
    }

    pub async fn stop_container(&self, container_id: &str) -> Result<()> {
        let request = format!("POST /containers/{}/stop HTTP/1.0\r\nHost: localhost\r\nConnection: close\r\n\r\n", container_id);
        self.send_request(&request).await?;
        Ok(())
    }

    pub async fn restart_container(&self, container_id: &str) -> Result<()> {
        let request = format!("POST /containers/{}/restart HTTP/1.0\r\nHost: localhost\r\nConnection: close\r\n\r\n", container_id);
        self.send_request(&request).await?;
        Ok(())
    }
}
