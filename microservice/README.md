# SnapTrade Service

A Go microservice for integrating with the SnapTrade API, providing brokerage account management, connection handling, and transaction synchronization.

## Overview

This service acts as a middleware layer between the Rust backend and the SnapTrade API, handling:
- User management (create, get, delete SnapTrade users)
- Connection management (initiate, check status, list, delete connections)
- Account management (list, get details, sync accounts)
- Transaction and holdings retrieval

## Prerequisites

- Docker and Docker Compose installed
- SnapTrade API credentials:
  - `SNAPTRADE_CLIENT_ID`
  - `SNAPTRADE_CONSUMER_KEY`

## Quick Start

### Using Docker Compose (Recommended)

1. Create a `.env` file in this directory:
```bash
SNAPTRADE_CLIENT_ID=your_client_id
SNAPTRADE_CONSUMER_KEY=your_consumer_key
PORT=8080
```

2. Build and run the service:
```bash
docker-compose up -d
```

3. Verify the service is running:
```bash
curl http://localhost:8080/health
```

### Using Docker directly

1. Build the image:
```bash
docker build -t snaptrade-service .
```

2. Run the container:
```bash
docker run -d \
  --name snaptrade-service \
  -p 8080:8080 \
  -e SNAPTRADE_CLIENT_ID=your_client_id \
  -e SNAPTRADE_CONSUMER_KEY=your_consumer_key \
  -e PORT=8080 \
  snaptrade-service
```

## Configuration

### Environment Variables

| Variable | Description | Required | Default |
|----------|-------------|----------|---------|
| `SNAPTRADE_CLIENT_ID` | SnapTrade API client ID | Yes | - |
| `SNAPTRADE_CONSUMER_KEY` | SnapTrade API consumer key | Yes | - |
| `PORT` | Service port | No | `8080` |

### Network Configuration

The service listens on `0.0.0.0:8080` by default, making it accessible from other containers and the host machine.

## Integration with Rust Backend

The Rust backend communicates with this service via HTTP. Configure the backend with:

```bash
SNAPTRADE_SERVICE_URL=http://snaptrade-service:8080
```

Or if running on the host (not in Docker network):
```bash
SNAPTRADE_SERVICE_URL=http://localhost:8080
```

### Docker Network Setup

If both services are running in Docker, ensure they're on the same network:

1. Create a shared network:
```bash
docker network create tradstry-network
```

2. Update `docker-compose.yml` to use the existing network:
```yaml
networks:
  tradstry-network:
    external: true
```

3. Connect your Rust backend container to the same network:
```bash
docker network connect tradstry-network your-rust-backend-container
```

## API Endpoints

### Health Check
- `GET /health` - Service health check

### User Management
- `POST /api/v1/users` - Create a SnapTrade user
- `GET /api/v1/users/:userId` - Get user details
- `DELETE /api/v1/users/:userId` - Delete a user

### Connection Management
- `POST /api/v1/connections/initiate` - Initiate a new connection
- `GET /api/v1/connections/:connectionId/status` - Get connection status
- `GET /api/v1/connections` - List all connections
- `DELETE /api/v1/connections/:connectionId` - Delete a connection

### Account Management
- `GET /api/v1/accounts` - List all accounts
- `GET /api/v1/accounts/:accountId` - Get account details
- `POST /api/v1/accounts/sync` - Sync accounts

### Transactions and Holdings
- `GET /api/v1/accounts/:accountId/transactions` - Get transactions
- `GET /api/v1/accounts/:accountId/holdings` - Get holdings
- `GET /api/v1/accounts/:accountId/holdings/options` - Get option positions

## Development

### Local Development (without Docker)

1. Install Go 1.24 or later
2. Install dependencies:
```bash
go mod download
```

3. Set environment variables:
```bash
export SNAPTRADE_CLIENT_ID=your_client_id
export SNAPTRADE_CONSUMER_KEY=your_consumer_key
export PORT=8080
```

4. Run the service:
```bash
go run main.go
```

### Building for Production

```bash
docker build -t snaptrade-service:latest .
```

## Troubleshooting

### Service not accessible from Rust backend

1. Check if the service is running:
```bash
docker ps | grep snaptrade-service
```

2. Check service logs:
```bash
docker logs snaptrade-service
```

3. Verify network connectivity:
```bash
# From Rust backend container
curl http://snaptrade-service:8080/health
```

4. Ensure both services are on the same Docker network

### Authentication errors

- Verify `SNAPTRADE_CLIENT_ID` and `SNAPTRADE_CONSUMER_KEY` are set correctly
- Check SnapTrade API documentation for credential requirements

## Health Check

The service includes a health check endpoint that returns:
```json
{
  "status": "ok",
  "service": "snaptrade-service"
}
```

Docker health checks are configured to use this endpoint automatically.

## CI/CD Pipeline

The service is automatically built and pushed to Docker Hub when you create a version tag.

### Creating a Release

1. Create and push a version tag:
```bash
git tag -a v1.0.0 -m "Release version 1.0.0"
git push origin v1.0.0
```

2. The GitHub Actions workflow will automatically:
   - Build the Docker image
   - Tag it with the version and `latest`
   - Push to Docker Hub (`johnsonf/snaptrade-service`)

### Deployment to VPS

After the image is built and pushed to Docker Hub, deploy it to your VPS:

#### Option 1: Deploy only SnapTrade service
```bash
cd backend/snaptrade-service
./deploy.sh
```

#### Option 2: Deploy all services (backend + snaptrade-service)
```bash
cd backend
./deploy-all.sh
```

The deployment script will:
- Pull the latest image from Docker Hub
- Restart the service with the new image
- Check service status and health
- Display recent logs

### Manual Deployment

If you need to manually deploy:

```bash
ssh -i ~/.ssh/id_ed25519_vps root@95.216.219.131
cd /opt/tradstry
docker compose pull snaptrade-service
docker compose up -d snaptrade-service
docker compose logs -f snaptrade-service
```

## License

Part of the Tradstry project.

