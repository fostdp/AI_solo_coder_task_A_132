@echo off
echo ============================================
echo   古代水运仪象台漏壶仿真系统 - 前端启动
echo ============================================
echo.
echo 正在启动前端HTTP服务器 (端口 3000)...
echo.
echo 如果提示找不到python，请先安装Python 3
echo 或使用任意静态文件服务器打开 frontend/index.html
echo.

where python >nul 2>nul
if %errorlevel%==0 (
    cd /d "%~dp0frontend"
    python -m http.server 3000
) else (
    echo 未找到Python，尝试使用PowerShell启动简单服务器...
    powershell -Command "$listener = New-Object System.Net.HttpListener; $listener.Prefixes.Add('http://localhost:3000/'); $listener.Start(); Write-Host '服务器已启动: http://localhost:3000'; while($listener.IsListening) { $context = $listener.GetContext(); $request = $context.Request; $response = $context.Response; $path = $request.Url.LocalPath; if($path -eq '/') { $path = '/index.html' }; $file = Join-Path '%~dp0frontend' $path.TrimStart('/'); if(Test-Path $file) { $content = [System.IO.File]::ReadAllBytes($file); $response.ContentLength64 = $content.Length; $response.OutputStream.Write($content, 0, $content.Length) } else { $response.StatusCode = 404 }; $response.Close() }"
)

pause
