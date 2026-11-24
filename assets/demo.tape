# Record from the repository root, with ast-pack.exe on PATH:
#   vhs assets/demo.tape
# Writes assets/demo.gif next to this file.
# This PC has Consolas and PowerShell. JetBrains Mono is not installed, and the
# Windows `bash` is WSL, which does not see ast-pack.exe.
# Timing is about 9s: typing plus two short sleeps.

Output assets/demo.gif

Set Shell powershell
Set FontFamily "Consolas"
Set FontSize 16
Set Width 1200
Set Height 700
Set Padding 20
Set TypingSpeed 35ms
Set Theme "Catppuccin Mocha"
Set Framerate 30

Type "ast-pack . --budget 4000 --stats"
Enter
Sleep 3s

Type `ast-pack unfold "src/utils.rs#parse_budget"`
Enter
Sleep 2.5s
