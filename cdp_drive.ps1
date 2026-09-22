# CDP driver v3 (raw WebSocket, no Origin header)
# Usage: powershell -File cdp_drive.ps1 <wsUrl> <jsExpression>
param(
  [Parameter(Mandatory=$true)][string]$WsUrl,
  [Parameter(Mandatory=$true)][string]$Js,
  [int]$TimeoutMs = 15000
)

$ErrorActionPreference = "Stop"

function Send-TextFrame($stream, $payload) {
  $data = [System.Text.Encoding]::UTF8.GetBytes($payload)
  $len = $data.Length
  $header = New-Object System.Collections.Generic.List[byte]
  $header.Add(0x81)
  if ($len -lt 126) {
    $header.Add([byte](0x80 -bor $len))
  } elseif ($len -lt 65536) {
    $header.Add([byte](0x80 -bor 126))
    $header.Add([byte](($len -shr 8) -band 0xFF))
    $header.Add([byte]($len -band 0xFF))
  } else {
    $header.Add([byte](0x80 -bor 127))
    for ($i = 7; $i -ge 0; $i--) { $header.Add([byte](($len -shr ($i * 8)) -band 0xFF)) }
  }
  $mask = New-Object byte[] 4
  (New-Object System.Random).NextBytes($mask)
  $header.AddRange($mask)
  $masked = New-Object byte[] $len
  for ($i = 0; $i -lt $len; $i++) { $masked[$i] = [byte]($data[$i] -bxor $mask[$i % 4]) }
  $stream.Write($header.ToArray(), 0, $header.Count)
  $stream.Write($masked, 0, $len)
  $stream.Flush()
}

function Receive-FullMessage($stream) {
  $sb = [System.Text.StringBuilder]::new()
  while ($true) {
    $b0 = $stream.ReadByte()
    if ($b0 -lt 0) { throw "connection closed" }
    $opcode = $b0 -band 0x0F
    $b1 = $stream.ReadByte()
    if ($b1 -lt 0) { throw "connection closed" }
    $masked = ($b1 -band 0x80) -ne 0
    $len = $b1 -band 0x7F
    if ($len -eq 126) {
      $len = ($stream.ReadByte() -shl 8) -bor $stream.ReadByte()
    } elseif ($len -eq 127) {
      $len = 0
      for ($i = 0; $i -lt 8; $i++) { $len = ($len -shl 8) -bor $stream.ReadByte() }
    }
    if ($opcode -eq 8) { throw "close frame received" }
    if ($opcode -eq 9) { continue }
    if ($opcode -eq 10) { continue }
    $maskKey = $null
    if ($masked) {
      $maskKey = New-Object byte[] 4
      for ($i = 0; $i -lt 4; $i++) { $maskKey[$i] = $stream.ReadByte() }
    }
    $buf = New-Object byte[] $len
    $read = 0
    while ($read -lt $len) {
      $n = $stream.Read($buf, $read, $len - $read)
      if ($n -le 0) { throw "connection closed during payload" }
      $read += $n
    }
    if ($masked) {
      for ($i = 0; $i -lt $len; $i++) { $buf[$i] = [byte]($buf[$i] -bxor $maskKey[$i % 4]) }
    }
    [void]$sb.Append([System.Text.Encoding]::UTF8.GetString($buf, 0, $len))
    if (($b0 -band 0x80) -ne 0) { break }
  }
  return $sb.ToString()
}

function Wait-CdpResponse($stream, $expectedId) {
  while ($true) {
    $msg = Receive-FullMessage $stream
    $parsed = $msg | ConvertFrom-Json
    if ($null -ne $parsed.id -and $parsed.id -eq $expectedId) {
      return $parsed
    }
  }
}

$uri = [Uri]$WsUrl
$hostport = "$($uri.Host):$($uri.Port)"

$tcp = [System.Net.Sockets.TcpClient]::new()
try {
  $tcp.Connect($uri.Host, $uri.Port)
  $stream = $tcp.GetStream()
  $stream.ReadTimeout = $TimeoutMs
  $stream.WriteTimeout = $TimeoutMs

  $key = [Convert]::ToBase64String([System.Text.Encoding]::ASCII.GetBytes("0123456789abcdef0123456789abcdef"))
  $req = "GET $($uri.PathAndQuery) HTTP/1.1`r`nHost: $hostport`r`nUpgrade: websocket`r`nConnection: Upgrade`r`nSec-WebSocket-Key: $key`r`nSec-WebSocket-Version: 13`r`n`r`n"
  $bytes = [System.Text.Encoding]::ASCII.GetBytes($req)
  $stream.Write($bytes, 0, $bytes.Length)
  $stream.Flush()

  $hsBytes = New-Object System.Collections.Generic.List[byte]
  while ($true) {
    $c = $stream.ReadByte()
    if ($c -lt 0) { throw "connection closed during handshake" }
    $hsBytes.Add([byte]$c)
    if ($hsBytes.Count -ge 4) {
      $n = $hsBytes.Count
      if ($hsBytes[$n-4] -eq 13 -and $hsBytes[$n-3] -eq 10 -and $hsBytes[$n-2] -eq 13 -and $hsBytes[$n-1] -eq 10) { break }
    }
  }
  $handshake = [System.Text.Encoding]::ASCII.GetString($hsBytes.ToArray())
  if ($handshake -notmatch "^HTTP/1\.1 101") {
    Write-Output "HANDSHAKE_FAILED:`n$handshake"
    exit 1
  }

  Send-TextFrame $stream '{"id":1,"method":"Runtime.enable","params":{}}'
  $null = Wait-CdpResponse $stream 1

  $expr = @{ id = 2; method = "Runtime.evaluate"; params = @{ expression = $Js; returnByValue = $true; awaitPromise = $true } } | ConvertTo-Json -Depth 10 -Compress
  Send-TextFrame $stream $expr
  $resp = Wait-CdpResponse $stream 2

  if ($resp.result.exceptionDetails) {
    Write-Output "JS_EXCEPTION: $($resp.result.result.description)"
    exit 1
  }
  if ($null -ne $resp.result.result.value) {
    Write-Output ($resp.result.result.value | ConvertTo-Json -Depth 20)
  } else {
    Write-Output "RAW_RESPONSE: $($resp | ConvertTo-Json -Depth 10 -Compress)"
  }
} catch {
  Write-Output "CDP_ERROR: $($_.Exception.Message)"
  exit 1
} finally {
  $tcp.Close()
}
