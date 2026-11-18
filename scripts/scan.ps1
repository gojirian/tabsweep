# Lists every tab in every Microsoft Edge browser window as JSON.
# Read-only: it walks the tab strip through UI Automation and touches nothing.
# Keep this file ASCII so Windows PowerShell 5.1 reads it the same as pwsh.

$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = New-Object System.Text.UTF8Encoding $false
Add-Type -AssemblyName UIAutomationClient, UIAutomationTypes

$AE = [System.Windows.Automation.AutomationElement]
$winCond = New-Object System.Windows.Automation.PropertyCondition($AE::ClassNameProperty, 'Chrome_WidgetWin_1')
$tabCond = New-Object System.Windows.Automation.PropertyCondition($AE::ClassNameProperty, 'EdgeTab')
$selection = [System.Windows.Automation.SelectionItemPattern]::Pattern

$items = New-Object System.Collections.Generic.List[object]
foreach ($w in $AE::RootElement.FindAll('Children', $winCond)) {
    $title = $w.Current.Name
    # Installed web apps (Gmail, Chat) share the window class but their titles
    # do not end in "Edge", and they have no tab strip worth cleaning.
    if ($title -notmatch 'Edge$') { continue }
    try { $process = Get-Process -Id $w.Current.ProcessId } catch { continue }
    if ($process.ProcessName -ne 'msedge') { continue }

    $index = 0
    foreach ($t in $w.FindAll('Descendants', $tabCond)) {
        $active = $false
        try { $active = $t.GetCurrentPattern($selection).Current.IsSelected } catch {}
        $items.Add([pscustomobject]@{
            hwnd   = [int64]$w.Current.NativeWindowHandle
            window = $title
            index  = $index
            name   = $t.Current.Name
            active = [bool]$active
        })
        $index++
    }
}

if ($items.Count -eq 0) { '[]' } else { ConvertTo-Json -InputObject $items.ToArray() -Compress }
