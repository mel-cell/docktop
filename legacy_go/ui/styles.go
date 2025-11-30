package ui

import "github.com/charmbracelet/lipgloss"

var (
	// Monochrome Colors
	colorBorder = lipgloss.Color("240") // Dark Gray
	colorText   = lipgloss.Color("252") // Light Gray
	colorWhite  = lipgloss.Color("255") // Pure White
	colorGray   = lipgloss.Color("240") // Gray
	colorBlack  = lipgloss.Color("232") // Near Black

	// Base Styles
	baseStyle = lipgloss.NewStyle().
			BorderStyle(lipgloss.RoundedBorder()).
			BorderForeground(colorBorder).
			Padding(0, 1)

	// Panel Styles
	listPanelStyle = baseStyle    // Copy by assignment
	monitorPanelStyle = baseStyle // Copy by assignment
	logPanelStyle = baseStyle     // Copy by assignment

	// Text Styles
	selectedItemStyle = lipgloss.NewStyle().
			Foreground(colorBlack).
			Background(colorWhite).
			Bold(true)
	
	statusRunning = lipgloss.NewStyle().Foreground(colorWhite).Bold(true)
	statusExited  = lipgloss.NewStyle().Foreground(colorGray)
	statusOther   = lipgloss.NewStyle().Foreground(colorText)
)
