#requires -Version 5.1
[CmdletBinding()]
param(
    [Parameter(Mandatory)] [string] $Server,
    [string] $Artifact = ".\dist\app.zip"
)

$ErrorActionPreference = "Stop"

Write-Verbose "copying $Artifact to $Server"
Copy-Item -Path $Artifact -Destination "\\$Server\deploy\" -Force

Invoke-Command -ComputerName $Server -ScriptBlock {
    Expand-Archive -Path "C:\deploy\app.zip" -DestinationPath "C:\app" -Force
    Restart-Service -Name "MyApp"
}
