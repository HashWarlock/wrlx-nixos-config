# CVM Troubleshooting Guide

## Service Startup Issues

### Agent API doesn't start
1. Check if cargo is available: `which cargo`
2. Try manual build: `cd /app/cvm-agent/agent-api && cargo build --release`
3. Check logs: `docker-compose logs | grep -i agent`

### Web UI doesn't start
1. Check if dist exists: `ls /app/cvm-agent/web-ui/dist`
2. Try manual build: `cd /app/cvm-agent/web-ui && npm install && npm run build`
3. Check logs: `docker-compose logs | grep -i web-ui`

### Whisper doesn't start
1. Check if model exists: `ls /nix/store/*/share/whisper-cpp/models/`
2. Set model: `export WHISPER_MODEL=base`
3. Check logs: `docker-compose logs | grep -i whisper`

## Port Conflicts
Default ports:
- 3000: Web UI
- 6080: noVNC
- 8080: Agent API (gRPC)
- 8082: Whisper
- 5900: VNC
- 2222: SSH (mapped from 22)

## Common Issues

### Connection reset on Web UI (port 3000)
- Check if web-ui.sh successfully started the Python HTTP server
- Verify dist directory exists at `/app/cvm-agent/web-ui/dist`
- Check stderr output in container logs

### gRPC calls fail
- Verify Agent API is running: `nc -z localhost 8080`
- Check proto file path in grpcurl command
- Use `-plaintext` flag for unencrypted connection

### VNC connection issues
- Default password: `changeme`
- noVNC web client: http://localhost:6080/vnc.html
- Direct VNC: port 5900

## Updating Container Code
The container clones from GitHub on first run. To update:
1. Set environment variable: `UPDATE_CODE=true`
2. Restart container: `docker-compose down && docker-compose up -d`
