package main

import (
	"fmt"
	"os"

	"docktop/ui"

	tea "github.com/charmbracelet/bubbletea"
)

func main() {
	m, err := ui.NewModel()
	if err != nil {
		fmt.Printf("❌ Error initializing: %v\n", err)
		os.Exit(1)
	}

	p := tea.NewProgram(m, tea.WithAltScreen())
	if _, err := p.Run(); err != nil {
		fmt.Printf("❌ Error running program: %v\n", err)
		os.Exit(1)
	}
}