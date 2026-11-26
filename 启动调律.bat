@echo off
setlocal
cd /d "%~dp0"
title TEMPER 调律

where node >nul 2>nul
if errorlevel 1 (
  echo [错误] 未找到 Node.js，请先安装 Node.js 20 或更高版本。
  pause
  exit /b 1
)
where npm >nul 2>nul
if errorlevel 1 (
  echo [错误] 未找到 npm，请检查 Node.js 安装。
  pause
  exit /b 1
)

rem 已经在运行就直接打开浏览器，避免重复启动。
powershell -NoProfile -Command "try{$c=New-Object Net.Sockets.TcpClient;$c.Connect('127.0.0.1',8787);$c.Close();exit 0}catch{exit 1}" >nul 2>nul
if not errorlevel 1 (
  echo 调律 TEMPER 已在运行，正在打开 http://127.0.0.1:8787 ...
  start "" "http://127.0.0.1:8787"
  exit /b 0
)

echo 正在启动 调律 TEMPER ...
echo 地址：http://127.0.0.1:8787
echo 首次启动需要编译，可能稍等片刻；就绪后会自动打开浏览器。
echo 保持此窗口开启，按 Ctrl+C 可停止服务。
echo.

rem 后台等待端口就绪，然后打开默认浏览器。
start "" /b powershell -NoProfile -WindowStyle Hidden -Command "for($i=0;$i -lt 600;$i++){try{$c=New-Object Net.Sockets.TcpClient;$c.Connect('127.0.0.1',8787);$c.Close();Start-Process 'http://127.0.0.1:8787';break}catch{Start-Sleep -Seconds 1}}"

call npm run dev
set "CODE=%ERRORLEVEL%"
echo.
if not "%CODE%"=="0" (
  echo 服务已退出，代码 %CODE%。
  pause
)
exit /b %CODE%
