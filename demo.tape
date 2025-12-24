# VHS Demo Tape for aptu
# Showcasing curated repository discovery, issue listing, and AI-powered triage

Output assets/demo.gif

Set FontFamily "Cascadia Code"
Set FontSize 14
Set Width 1200
Set Height 700
Set TypingSpeed 50ms
Set Theme "Catppuccin Mocha"

Hide
Type "export PATH=$PWD/target/release:$PATH"
Enter
Show

# Scene 1: Help command
Type "aptu --help"
Enter
Sleep 2s

# Scene 2: Repository listing
Type "clear"
Enter
Type "aptu repo list"
Enter
Sleep 2s

# Scene 3: Issue listing
Type "clear"
Enter
Type "aptu issue list block/goose"
Enter
Sleep 2s

# Scene 4: AI-powered triage with variable response time
Type "clear"
Enter
Type "aptu issue triage --force --dry-run clouatre-labs/aptu#209"
Enter
Sleep 15s
