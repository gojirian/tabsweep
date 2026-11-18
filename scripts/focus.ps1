# Switches Edge to one tab and brings its window to the front.
#
# The tab is found by its place in the strip at scan time and checked against
# the title it had then, so a strip that has changed since is refused rather
# than showing the wrong tab. Prints OK, or FAIL <reason>.
# Keep this file ASCII so Windows PowerShell 5.1 reads it the same as pwsh.

param(
    [Parameter(Mandatory = $true)][int64]$Hwnd,
    [Parameter(Mandatory = $true)][int]$Index,
    [Parameter(Mandatory = $true)][string]$TitlePath
)

$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = New-Object System.Text.UTF8Encoding $false
Add-Type -AssemblyName UIAutomationClient, UIAutomationTypes
Add-Type @'
using System; using System.Runtime.InteropServices;
public class TabSweepFocus {
  [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
  [DllImport("user32.dll")] public static extern bool IsIconic(IntPtr h);
  [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr h, int cmd);
  [DllImport("user32.dll")] public static extern void keybd_event(byte k, byte s, uint f, UIntPtr e);
}
'@

# The title comes in a file because a command line mangles quotes and
# non-ASCII characters.
$title = (Get-Content -Raw -Encoding UTF8 -LiteralPath $TitlePath).Trim()

$AE = [System.Windows.Automation.AutomationElement]
$tabCond = New-Object System.Windows.Automation.PropertyCondition($AE::ClassNameProperty, 'EdgeTab')
$handle = [IntPtr]$Hwnd

try { $window = $AE::FromHandle($handle) } catch { 'FAIL that Edge window is closed'; exit }
$tabs = @($window.FindAll('Descendants', $tabCond))
$tab = $null
if ($Index -lt $tabs.Count) { $tab = $tabs[$Index] }
if ($null -eq $tab -or [string]::IsNullOrEmpty($title) -or
    -not $tab.Current.Name.StartsWith($title, [StringComparison]::Ordinal)) {
    'FAIL the tab has moved since the scan. Press Rescan.'
    exit
}

$tab.GetCurrentPattern([System.Windows.Automation.SelectionItemPattern]::Pattern).Select()
if ([TabSweepFocus]::IsIconic($handle)) { [TabSweepFocus]::ShowWindow($handle, 9) | Out-Null }
# Windows only lets a process take the foreground after input; a tap of Alt counts.
[TabSweepFocus]::keybd_event(0x12, 0, 0, [UIntPtr]::Zero)
[TabSweepFocus]::keybd_event(0x12, 0, 2, [UIntPtr]::Zero)
[TabSweepFocus]::SetForegroundWindow($handle) | Out-Null
'OK'
