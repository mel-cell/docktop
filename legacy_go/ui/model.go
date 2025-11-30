package ui

import (
	"fmt"
	"strings"
	"sync"
	"time"

	"docktop/docker"

	tea "github.com/charmbracelet/bubbletea"
	"github.com/charmbracelet/lipgloss"
	"github.com/docker/docker/api/types"
)



type Model struct {
	dockerClient *docker.DockerClient
	containers   []types.Container
	stats        map[string]types.StatsJSON
	cpuHistory   map[string][]float64
	inspectData  *types.ContainerJSON
	logs         string
	cursor       int
	activePanel  int // 0: List, 1: Logs
	logOffset    int // 0 means bottom (auto-scroll), >0 means scrolled up
	err          error
	statusMsg    string
	width        int
	height       int
}

type ContainerData struct {
	List  []types.Container
	Stats map[string]types.StatsJSON
}

type TickMsg time.Time

func NewModel() (*Model, error) {
	client, err := docker.NewDockerClient()
	if err != nil {
		return nil, err
	}

	return &Model{
		dockerClient: client,
		stats:        make(map[string]types.StatsJSON),
		cpuHistory:   make(map[string][]float64),
	}, nil
}

func (m Model) Init() tea.Cmd {
	return tea.Batch(
		m.fetchContainers(),
		m.tick(),
	)
}



// View
func (m Model) View() string {
	if m.err != nil {
		return fmt.Sprintf("Error: %v", m.err)
	}

	// 1. Minimum Size Check
	if m.width < 80 || m.height < 24 {
		msg := fmt.Sprintf("⚠️  Terminal too small!\n\nNeed: 80x24\nCurrent: %dx%d", m.width, m.height)
		return lipgloss.Place(m.width, m.height, lipgloss.Center, lipgloss.Center, 
			lipgloss.NewStyle().Foreground(lipgloss.Color("196")).Bold(true).Render(msg))
	}

	// 2. Calculate Layout
	headerHeight := 1
	footerHeight := 2
	
	// Total available height for the main content
	// We subtract an extra 1 line for safety to absolutely prevent scrolling
	availableHeight := m.height - headerHeight - footerHeight - 1
	
	if availableHeight < 10 { availableHeight = 10 }

	// Calculate Widths
	// We want left + right = m.width exactly.
	leftWidth := int(float64(m.width) * 0.35) 
	rightWidth := m.width - leftWidth

	// 3. Render Components
	header := m.renderHeader()
	footer := m.renderFooter()

	// Styles for active/inactive panels
	listStyle := listPanelStyle
	logStyle := logPanelStyle
	detailStyle := monitorPanelStyle
	
	if m.activePanel == 0 {
		listStyle = listStyle.BorderForeground(colorWhite)
		logStyle = logStyle.BorderForeground(colorGray)
	} else {
		listStyle = listStyle.BorderForeground(colorGray)
		logStyle = logStyle.BorderForeground(colorWhite)
	}

	// --- Left Panel: List ---
	// We enforce strict width/height on the style
	listStyle = listStyle.Width(leftWidth - 2).Height(availableHeight - 2) // -2 for borders
	
	// Inner content size
	listInnerWidth := leftWidth - 4 // -2 border, -2 padding
	listInnerHeight := availableHeight - 2 // -2 border
	
	listContent := m.renderContainerList(listInnerWidth, listInnerHeight)
	listRendered := listStyle.Render(listContent)
	
	// Force exact size using Place (Absolute Positioning)
	listFixed := lipgloss.Place(leftWidth, availableHeight, lipgloss.Left, lipgloss.Top, listRendered)

	// --- Right Panel: Details (Top) + Logs (Bottom) ---
	detailsHeight := 10 
	logsHeight := availableHeight - detailsHeight
	
	// Details Style
	detailStyle = detailStyle.Width(rightWidth - 2).Height(detailsHeight - 2)
	detailsInnerWidth := rightWidth - 4
	detailsContent := m.renderDetails(detailsInnerWidth)
	detailsRendered := detailStyle.Render(detailsContent)
	
	// Logs Style
	logStyle = logStyle.Width(rightWidth - 2).Height(logsHeight - 2)
	logsInnerWidth := rightWidth - 4
	logsInnerHeight := logsHeight - 2
	logsContent := m.renderLogs(logsInnerWidth, logsInnerHeight)
	logsRendered := logStyle.Render(logsContent)
	
	// Combine Right Panel
	rightStack := lipgloss.JoinVertical(lipgloss.Left, detailsRendered, logsRendered)
	
	// Force exact size for right panel
	rightFixed := lipgloss.Place(rightWidth, availableHeight, lipgloss.Left, lipgloss.Top, rightStack)

	// --- Combine All ---
	mainBody := lipgloss.JoinHorizontal(lipgloss.Top, listFixed, rightFixed)
	
	// Force Header/Footer Size
	headerFixed := lipgloss.Place(m.width, headerHeight, lipgloss.Left, lipgloss.Top, header)
	footerFixed := lipgloss.Place(m.width, footerHeight, lipgloss.Left, lipgloss.Top, footer)
	
	return lipgloss.JoinVertical(lipgloss.Left, headerFixed, mainBody, footerFixed)
}

// --- RENDERERS ---

func (m Model) renderHeader() string {
	running := 0
	stopped := 0
	for _, c := range m.containers {
		if c.State == "running" {
			running++
		} else {
			stopped++
		}
	}

	titleText := "DOCKTOP PRO"
	title := lipgloss.NewStyle().
		Foreground(colorBlack).
		Background(colorWhite).
		Padding(0, 1).
		Bold(true).
		Render(titleText)

	statsText := fmt.Sprintf("Running: %d | Stopped: %d", running, stopped)
	statsStyle := lipgloss.NewStyle().Foreground(colorText).Render(statsText)

	// Spacer calculation
	totalContentWidth := lipgloss.Width(title) + lipgloss.Width(statsStyle)
	spacerWidth := m.width - totalContentWidth
	
	if spacerWidth < 0 {
		spacerWidth = 0
	}
	
	spacer := strings.Repeat(" ", spacerWidth)

	return fmt.Sprintf("%s%s%s", title, spacer, statsStyle)
}

func (m Model) renderFooter() string {
	help := "j/k: Nav • Tab: Switch Panel • r: Restart • s: Stop • q: Quit"
	
	// Truncate if too long
	if len(help) > m.width {
		help = help[:m.width-1] + "…"
	}

	return lipgloss.NewStyle().
		Foreground(colorGray).
		PaddingTop(1).
		Render(help)
}

func (m Model) renderContainerList(w, h int) string {
	s := ""
	
	// Scroll logic
	start := 0
	end := len(m.containers)
	if m.cursor >= h {
		start = m.cursor - h + 1
	}
	if end > start + h {
		end = start + h
	}

	for i := start; i < end; i++ {
		c := m.containers[i]
		
		// Symbol & Style
		symbol := "○"
		style := statusOther
		
		switch c.State {
		case "running":
			symbol = "●"
			style = statusRunning
		case "exited":
			symbol = "○"
			style = statusExited
		}

		// Cursor
		cursor := " "
		if m.cursor == i {
			cursor = "│" // Modern cursor style
			style = selectedItemStyle
		}

		// Name Truncation
		name := "Unknown"
		if len(c.Names) > 0 {
			name = c.Names[0][1:]
		}
		
		// Strict truncation to prevent wrapping
		// Available width = w
		// Used: cursor(1) + space(1) + symbol(1) + space(1) = 4 chars
		maxNameLen := w - 4
		if maxNameLen < 1 { maxNameLen = 1 }
		
		if len(name) > maxNameLen {
			name = name[:maxNameLen-1] + "…"
		}

		row := fmt.Sprintf("%s %s %s", cursor, symbol, name)
		s += style.Render(row) + "\n"
	}
	return strings.TrimRight(s, "\n")
}

func (m Model) renderDetails(w int) string {
	if m.inspectData == nil {
		return "Select a container to view details..."
	}

	data := m.inspectData
	
	// ID & Image
	id := data.ID
	if len(id) > 8 { id = id[:8] }
	image := data.Config.Image
	
	// Truncate Image
	if len(image) > w - 10 { // Rough estimate for label width
		image = image[:w-13] + "..."
	}

	// State & IP
	state := "Unknown"
	if data.State != nil {
		state = data.State.Status
	}
	ip := "N/A"
	if data.NetworkSettings != nil {
		ip = data.NetworkSettings.IPAddress
	}

	// Stats (CPU/Mem)
	cpu := "0%"
	mem := "0MB"
	if stats, ok := m.stats[data.ID]; ok {
		cpu = fmt.Sprintf("%.1f%%", docker.CalculateCPUPercent(stats))
		mem = docker.FormatMemory(stats)
	}

	// Layout
	labelStyle := lipgloss.NewStyle().Foreground(colorGray)
	valueStyle := lipgloss.NewStyle().Foreground(colorText)

	rows := []string{
		fmt.Sprintf("%s : %s", labelStyle.Render("ID     "), valueStyle.Render(id)),
		fmt.Sprintf("%s : %s", labelStyle.Render("Image  "), valueStyle.Render(image)),
		fmt.Sprintf("%s : %s", labelStyle.Render("State  "), valueStyle.Render(state)),
		fmt.Sprintf("%s : %s", labelStyle.Render("IP     "), valueStyle.Render(ip)),
		fmt.Sprintf("%s : %s", labelStyle.Render("CPU/Mem"), valueStyle.Render(fmt.Sprintf("%s / %s", cpu, mem))),
	}

	return strings.Join(rows, "\n")
}

func (m Model) Update(msg tea.Msg) (tea.Model, tea.Cmd) {
	var cmd tea.Cmd
	var cmds []tea.Cmd

	switch msg := msg.(type) {
	case tea.KeyMsg:
		switch msg.String() {
		case "q", "ctrl+c":
			return m, tea.Quit

		case "tab":
			m.activePanel = (m.activePanel + 1) % 2
			// Reset log offset when switching away from logs panel
			if m.activePanel == 0 {
				m.logOffset = 0
			}
			return m, nil

		case "r":
			if m.activePanel == 0 && len(m.containers) > 0 {
				selectedID := m.containers[m.cursor].ID
				m.statusMsg = "Restarting " + selectedID[:8] + "..."
				return m, m.restartContainer(selectedID)
			}

		case "s":
			if m.activePanel == 0 && len(m.containers) > 0 {
				selectedID := m.containers[m.cursor].ID
				m.statusMsg = "Stopping " + selectedID[:8] + "..."
				return m, m.stopContainer(selectedID)
			}

		case "u": // Up/Start
			if m.activePanel == 0 && len(m.containers) > 0 {
				selectedID := m.containers[m.cursor].ID
				m.statusMsg = "Starting " + selectedID[:8] + "..."
				return m, m.startContainer(selectedID)
			}

		case "up", "k":
			if m.activePanel == 0 {
				// List Navigation
				if m.cursor > 0 {
					m.cursor--
					// Fetch details for new selection
					if len(m.containers) > 0 {
						return m, m.fetchDetails(m.containers[m.cursor].ID)
					}
				}
			} else {
				// Log Scrolling: Up Arrow -> Go to Newer (Decrease Offset)
				if m.logOffset > 0 {
					m.logOffset--
				}
			}

		case "down", "j":
			if m.activePanel == 0 {
				// List Navigation
				if m.cursor < len(m.containers)-1 {
					m.cursor++
					// Fetch details for new selection
					if len(m.containers) > 0 {
						return m, m.fetchDetails(m.containers[m.cursor].ID)
					}
				}
			} else {
				// Log Scrolling: Down Arrow -> Go to Older (Increase Offset)
				m.logOffset++
			}
		
		case "window-size": // Handle window resize if needed
			// bubbletea handles this automatically via WindowSizeMsg, but good to have hook
		}

	case tea.WindowSizeMsg:
		m.width = msg.Width
		m.height = msg.Height

	case *types.ContainerJSON:
		m.inspectData = msg
		return m, nil

	case string: // For messages like "Restarted XXXXX" or "LOGS:..."
		if strings.HasPrefix(msg, "LOGS:") {
			m.logs = strings.TrimPrefix(msg, "LOGS:")
			m.logOffset = 0 // Reset log offset when new logs arrive
		} else {
			m.statusMsg = msg
		}
		return m, nil

	case error:
		m.statusMsg = fmt.Sprintf("Error: %v", msg)
		return m, nil

	case ContainerData:
		m.containers = msg.List
		m.stats = msg.Stats
		
		// Update History (Conveyor Belt Logic)
		for id, s := range msg.Stats {
			cpu := docker.CalculateCPUPercent(s)
			if _, ok := m.cpuHistory[id]; !ok {
				m.cpuHistory[id] = make([]float64, 0)
			}
			m.cpuHistory[id] = append(m.cpuHistory[id], cpu)
			
			// Keep last 30 data points
			if len(m.cpuHistory[id]) > 30 {
				m.cpuHistory[id] = m.cpuHistory[id][1:]
			}
		}

		// If no container is selected, or selected container is gone, select first
		if m.cursor >= len(m.containers) {
			m.cursor = 0
		}
		// We don't need to re-fetch details here every tick, it causes flickering/lag
		// Only fetch details if we don't have them yet or if user navigates
		
		cmds = append(cmds, m.tick()) // Start the next tick
		return m, tea.Batch(cmds...)

	case TickMsg:
		// Fetch containers and stats periodically
		cmds = append(cmds, m.fetchContainers())
		return m, tea.Batch(cmds...)
	}

	return m, cmd
}

func (m Model) renderLogs(w, h int) string {
	if m.logs == "" {
		return lipgloss.NewStyle().Foreground(colorGray).Render("No logs available.")
	}

	rawLines := strings.Split(m.logs, "\n")
	var lines []string
	for _, l := range rawLines {
		if strings.TrimSpace(l) != "" {
			// Clean up some common docker log prefixes if needed, or just keep as is
			lines = append(lines, l)
		}
	}
	
	totalLines := len(lines)
	if totalLines == 0 {
		return lipgloss.NewStyle().Foreground(colorGray).Render("No logs available.")
	}

	// Status Bar
	status := fmt.Sprintf(" Total: %d | Offset: %d (↓ Older / ↑ Newer) ", totalLines, m.logOffset)
	statusStyle := lipgloss.NewStyle().
		Background(colorWhite).
		Foreground(colorBlack).
		Width(w).
		Align(lipgloss.Right).
		Render(status)
	
	// Available height for logs is h - 1 (for status bar)
	logHeight := h - 1
	if logHeight < 1 { logHeight = 1 }

	s := ""
	
	// We want to show 'logHeight' lines, starting from the newest (end of slice)
	// offset 0 means start from the very last line (Newest)
	
	startIdx := totalLines - 1 - m.logOffset
	
	// Safety check for offset
	if startIdx < 0 { 
		// If offset is too large, just show from the oldest
		// But usually we want to clamp offset. For now let's just handle display.
		// Better: Clamp offset in Update, but here just handle display.
		startIdx = -1 // Loop won't run
	}

	count := 0
	for i := startIdx; i >= 0 && count < logHeight; i-- {
		line := lines[i]
		
		// Strict truncation
		if len(line) > w {
			line = line[:w-1] + "…"
		}
		s += line + "\n"
		count++
	}
	
	// Fill remaining lines with empty space to keep status bar at bottom
	if count < logHeight {
		s += strings.Repeat("\n", logHeight-count)
	}
	
	return strings.TrimRight(s, "\n") + "\n" + statusStyle
}

// Actions
func (m Model) restartContainer(id string) tea.Cmd {
	return func() tea.Msg {
		if err := m.dockerClient.RestartContainer(id); err != nil {
			return err
		}
		return "Restarted " + id[:8]
	}
}

func (m Model) stopContainer(id string) tea.Cmd {
	return func() tea.Msg {
		if err := m.dockerClient.StopContainer(id); err != nil {
			return err
		}
		return "Stopped " + id[:8]
	}
}

func (m Model) startContainer(id string) tea.Cmd {
	return func() tea.Msg {
		if err := m.dockerClient.StartContainer(id); err != nil {
			return err
		}
		return "Started " + id[:8]
	}
}

// Commands
func (m Model) fetchDetails(id string) tea.Cmd {
	return tea.Batch(
		func() tea.Msg {
			info, err := m.dockerClient.InspectContainer(id)
			if err != nil {
				return err
			}
			return &info
		},
		func() tea.Msg {
			logs, err := m.dockerClient.GetContainerLogs(id)
			if err != nil {
				return err
			}
			return "LOGS:" + logs
		},
	)
}

// Commands
func (m Model) fetchContainers() tea.Cmd {
	return func() tea.Msg {
		containers, err := m.dockerClient.ListContainers()
		if err != nil {
			return err
		}

		stats := make(map[string]types.StatsJSON)
		var mu sync.Mutex
		var wg sync.WaitGroup

		for _, c := range containers {
			if c.State == "running" {
				wg.Add(1)
				go func(id string) {
					defer wg.Done()
					s, err := m.dockerClient.GetContainerStats(id)
					if err == nil {
						mu.Lock()
						stats[id] = s
						mu.Unlock()
					}
				}(c.ID)
			}
		}
		wg.Wait()

		return ContainerData{List: containers, Stats: stats}
	}
}

func (m Model) tick() tea.Cmd {
	return tea.Tick(time.Second, func(t time.Time) tea.Msg {
		return TickMsg(t)
	})
}
