use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::UnixStream;
use serde::Deserialize;
use anyhow::Result;

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
}

#[derive(Debug, Deserialize, Clone)]
pub struct ContainerStats {
    pub cpu_stats: CpuStats,
    pub precpu_stats: CpuStats,
    pub memory_stats: MemoryStats,
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
    pub usage: u64,
    pub limit: u64,
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
            return Err(anyhow::anyhow!("Invalid response from Docker daemon"));
        }
        
        Ok(parts[1].to_string())
    }

    pub async fn list_containers(&self) -> Result<Vec<Container>> {
        let request = "GET /containers/json?all=true HTTP/1.0\r\nHost: localhost\r\n\r\n";
        let body = self.send_request(request).await?;
        let containers: Vec<Container> = serde_json::from_str(&body)?;
        Ok(containers)
    }

    pub async fn get_stats(&self, container_id: &str) -> Result<ContainerStats> {
        let request = format!("GET /containers/{}/stats?stream=false HTTP/1.0\r\nHost: localhost\r\n\r\n", container_id);
        let body = self.send_request(&request).await?;
        let stats: ContainerStats = serde_json::from_str(&body)?;
        Ok(stats)
    }
}
