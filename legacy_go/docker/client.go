package docker

import (
	"context"
	"encoding/json"
	"fmt"
	"io"
	"strings"

	"github.com/docker/docker/api/types"
	"github.com/docker/docker/api/types/container"
	"github.com/docker/docker/client"
)

type DockerClient struct {
	api *client.Client
}

func NewDockerClient() (*DockerClient, error) {
	// FromEnv: Baca settingan dari Environment (penting buat Arch/Podman nanti)
	// WithAPIVersionNegotiation: Otomatis cari versi API yang cocok biar gak error version mismatch
	cli, err := client.NewClientWithOpts(client.FromEnv, client.WithAPIVersionNegotiation())
	if err != nil {
		return nil, fmt.Errorf("gagal connect ke docker: %w", err)
	}

	return &DockerClient{api: cli}, nil
}

// ListContainers mengambil semua container (Running & Exited)
func (dc *DockerClient) ListContainers() ([]types.Container, error) {
	// All: true artinya tampilkan juga container yang mati (Exited)
	options := types.ContainerListOptions{All: true}

	containers, err := dc.api.ContainerList(context.Background(), options)
	if err != nil {
		return nil, fmt.Errorf("gagal ambil list container: %w", err)
	}

	return containers, nil
}

// GetContainerStats mengambil statistik resource container (CPU, RAM)
// stream: false artinya kita cuma ambil snapshot saat ini (bukan streaming terus menerus)
func (dc *DockerClient) GetContainerStats(containerID string) (types.StatsJSON, error) {
	resp, err := dc.api.ContainerStats(context.Background(), containerID, false)
	if err != nil {
		return types.StatsJSON{}, err
	}
	defer resp.Body.Close()

	var statsJSON types.StatsJSON
	if err := json.NewDecoder(resp.Body).Decode(&statsJSON); err != nil {
		return types.StatsJSON{}, err
	}

	return statsJSON, nil
}

func CalculateCPUPercent(stats types.StatsJSON) float64 {
	cpuPercent := 0.0
	cpuDelta := float64(stats.CPUStats.CPUUsage.TotalUsage) - float64(stats.PreCPUStats.CPUUsage.TotalUsage)
	systemDelta := float64(stats.CPUStats.SystemUsage) - float64(stats.PreCPUStats.SystemUsage)

	if systemDelta > 0.0 && cpuDelta > 0.0 {
		// PercpuUsage might be empty on cgroups v2, use OnlineCPUs
		cpus := float64(stats.CPUStats.OnlineCPUs)
		if cpus == 0.0 {
			cpus = float64(len(stats.CPUStats.CPUUsage.PercpuUsage))
		}
		
		cpuPercent = (cpuDelta / systemDelta) * cpus * 100.0
	}
	return cpuPercent
}

func FormatMemory(stats types.StatsJSON) string {
	usage := float64(stats.MemoryStats.Usage)
	limit := float64(stats.MemoryStats.Limit)
	
	usageMB := usage / 1024 / 1024
	limitMB := limit / 1024 / 1024
	
	percent := 0.0
	if limit > 0 {
		percent = (usage / limit) * 100.0
	}

	return fmt.Sprintf("%.1fMB / %.1fMB (%.1f%%)", usageMB, limitMB, percent)
}

func (dc *DockerClient) RestartContainer(containerID string) error {
	timeout := 10
	return dc.api.ContainerRestart(context.Background(), containerID, container.StopOptions{Timeout: &timeout})
}

func (dc *DockerClient) StopContainer(containerID string) error {
	timeout := 10
	return dc.api.ContainerStop(context.Background(), containerID, container.StopOptions{Timeout: &timeout})
}

func (dc *DockerClient) StartContainer(containerID string) error {
	return dc.api.ContainerStart(context.Background(), containerID, types.ContainerStartOptions{})
}

// InspectContainer mengambil detail lengkap (IP, Env, Mounts)
func (dc *DockerClient) InspectContainer(containerID string) (types.ContainerJSON, error) {
	return dc.api.ContainerInspect(context.Background(), containerID)
}

// GetContainerLogs mengambil 20 baris log terakhir
func (dc *DockerClient) GetContainerLogs(containerID string) (string, error) {
	options := types.ContainerLogsOptions{ShowStdout: true, ShowStderr: true, Tail: "300"}
	out, err := dc.api.ContainerLogs(context.Background(), containerID, options)
	if err != nil {
		return "", err
	}
	defer out.Close()

	// Baca log (simple read, idealnya pakai stdcopy tapi untuk text biasa cukup ini dulu)
	buf := new(strings.Builder)
	_, err = io.Copy(buf, out)
	if err != nil {
		return "", err
	}
	return buf.String(), nil
}