# Closes the tabs listed in a JSON file: [{ "hwnd": 1, "index": 3, "title": "..." }].
#
# Edge has no API for closing a tab from outside, so each tab is selected
# through UI Automation and closed with Ctrl+W. Ctrl+W is only sent after
# checking that the Edge window is in front and the wanted tab is the selected
# one, so a stray keypress cannot land anywhere else.
#
# Prints one line per tab: CLOSED, SKIP <reason>, or a final STOP <reason>.
# Keep this file ASCII so Windows PowerShell 5.1 reads it the same as pwsh.

param([Parameter(Mandatory = $true)][string]$TargetsPath)

$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = New-Object System.Text.UTF8Encoding $false
Add-Type -AssemblyName UIAutomationClient, UIAutomationTypes, System.Windows.Forms
Add-Type @'
using System; using System.Runtime.InteropServices;
public class TabSweepWin32 {
  [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
  [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
  [DllImport("user32.dll")] public static extern void keybd_event(byte k, byte s, uint f, UIntPtr e);
}
'@

function Say([string]$line) {
    [Console]::Out.WriteLine($line)
    [Console]::Out.Flush()
}

$AE = [System.Windows.Automation.AutomationElement]
$tabCond = New-Object System.Windows.Automation.PropertyCondition($AE::ClassNameProperty, 'EdgeTab')
$selection = [System.Windows.Automation.SelectionItemPattern]::Pattern

# Windows PowerShell 5.1 hands the whole JSON array on as one object; the
# extra pipeline step unrolls it.
$targets = @((Get-Content -Raw -Encoding UTF8 -LiteralPath $TargetsPath | ConvertFrom-Json) | ForEach-Object { $_ })
$stopped = $false

foreach ($group in ($targets | Group-Object hwnd)) {
    if ($stopped) { break }
    $hwnd = [IntPtr][int64]$group.Name
    try { $window = $AE::FromHandle($hwnd) } catch {
        foreach ($target in $group.Group) { Say 'SKIP window is gone' }
        continue
    }
    $tabs = @($window.FindAll('Descendants', $tabCond))

    # Pick every element before closing anything, so indexes from the scan
    # still mean the same tabs.
    $picked = @()
    foreach ($target in $group.Group) {
        $tab = $null
        if ($target.index -lt $tabs.Count) { $tab = $tabs[$target.index] }
        if ($null -eq $tab -or [string]::IsNullOrEmpty($target.title) -or
            -not $tab.Current.Name.StartsWith($target.title, [StringComparison]::Ordinal)) {
            Say 'SKIP tab moved or changed since the scan'
            continue
        }
        $picked += $tab
    }

    $count = $tabs.Count
    foreach ($tab in $picked) {
        try {
            $select = $tab.GetCurrentPattern($selection)
            $ready = $false
            foreach ($attempt in 1..3) {
                $select.Select()
                # Windows only lets a process take the foreground after input;
                # a tap of Alt counts.
                [TabSweepWin32]::keybd_event(0x12, 0, 0, [UIntPtr]::Zero)
                [TabSweepWin32]::keybd_event(0x12, 0, 2, [UIntPtr]::Zero)
                [TabSweepWin32]::SetForegroundWindow($hwnd) | Out-Null
                Start-Sleep -Milliseconds 600
                if ([TabSweepWin32]::GetForegroundWindow() -eq $hwnd -and $select.Current.IsSelected) {
                    $ready = $true
                    break
                }
            }
            if (-not $ready) {
                Say 'SKIP could not bring the tab to the front'
                continue
            }

            [System.Windows.Forms.SendKeys]::SendWait('^w')

            $closed = $false
            foreach ($wait in 1..20) {
                Start-Sleep -Milliseconds 400
                $now = $window.FindAll('Descendants', $tabCond).Count
                if ($now -eq $count - 1) { $closed = $true; break }
            }
            if (-not $closed) {
                # A page asking "Leave site?" keeps its tab. Sending more keys
                # now would answer that dialog, so stop here.
                Say 'STOP a tab did not close (it may be asking to leave the page)'
                $stopped = $true
                break
            }
            $count = $now
            Say 'CLOSED'
        } catch {
            Say 'SKIP tab went away'
        }
    }
}
