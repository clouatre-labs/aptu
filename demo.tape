# SPDX-License-Identifier: Apache-2.0
# SPDX-FileCopyrightText: 2025 Clouatre Labs

# VHS demo script for Aptu - AI-Powered Triage Utility
# Showcases curated repository discovery, issue listing, and AI-powered triage

Output "assets/demo.gif"

Set FontFamily "Cascadia Code"
Set FontSize 22
Set Width 1200
Set Height 800
Set TypingSpeed 50ms
Set Theme "Catppuccin Mocha"

# Setup PATH to use local build (hidden from recording)
Hide
Type "export PATH=$PWD/target/release:$PATH"
Enter
Show

# Scene 1: Help command
Type "aptu --help"
Enter
Sleep 2s

# Scene 2: Repository listing
Type "aptu repo list"
Enter
Sleep 2s

# Scene 3: Issue listing
Type "aptu issue list block/goose"
Enter
Sleep 2s

# Scene 4: AI-powered triage (clear for full visibility)
Type "clear"
Enter
Type "aptu issue triage --force --dry-run block/goose#6216"
Enter
Sleep 15s
