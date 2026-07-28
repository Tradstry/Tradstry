# Production Deployment & CI/CD Guide

This guide documents the production deployment process, CI/CD pipelines, and scripts used to manage the Tradstry backend and microservices on the VPS.

## Architecture

The backend consists of two main services deployed via Docker:
1. **Main Backend (Rust)**: Handles API requests, auth, and database interactions.
2. **SnapTrade Service (Go)**: Microservice for brokerage integrations.

Both services are containerized and deployed to a VPS using Docker Compose.

## CI/CD Pipeline

The project uses GitHub Actions for Continuous Integration and Continuous Delivery.

### Workflows

#### 1. Continuous Integration (`ci-cd.yml`, `pr-checks.yml`)
*   **Triggers:** Push to `main`/`develop`, Pull Requests.
*   **Actions:**
    *   **Frontend:** Installs dependencies (Bun), runs type-checks, and builds the Next.js app.
    *   **Backend:** Installs Rust, caches dependencies, runs `clippy` (linting), and verifies the release build.
    *   **Security:** Runs basic security audits.

#### 2. Continuous Delivery (`cd-release.yml`)
*   **Triggers:** Pushing a version tag (e.g., `v1.0.0`).
*   **Actions:**
    *   **Build:** Builds Docker images for both Backend and SnapTrade service.
    *   **Push:** Pushes images to Docker Hub:
        *   `johnsonf/tradstry-backend:latest` & `:{tag}`
        *   `johnsonf/snaptrade-service:latest` & `:{tag}`

### Configuring GitHub Secrets

To make the CD pipeline work, you must configure the following **Secrets** in your GitHub repository settings:

| Secret Name | Description |
|-------------|-------------|
| `DOCKERHUB_USERNAME` | Your Docker Hub username (e.g., `johnsonf`). |
| `DOCKERHUB_TOKEN` | Docker Hub Access Token (preferred over password). |

> **Note:** Ensure the image names in `.github/workflows/cd-release.yml` match your Docker Hub repository names.

---

## 🛠️ Deployment Scripts

Deployment to the VPS is currently triggered via local shell scripts that SSH into the server. These scripts rely on the Docker images built by the CD pipeline.

### Prerequisites
# Replace the `root@95.216.219.131` with your VPS IP address 
*   **SSH Access:** You must have SSH access to the VPS (`root@95.216.219.131`) configured with an SSH key at `~/.ssh/id_ed25519_vps`.
*   **Docker Images:** Ensure the CD pipeline has successfully run and pushed new images to Docker Hub.
*   **Environment Files:**
    *   `backend/.env.production` (for the Rust backend)
    *   `backend/snaptrade-service/.env` (for the Go service)

### Available Scripts

#### 1. Deploy All (`backend/deploy-all.sh`)
Deploys both the main backend and the SnapTrade service sequentially.

```bash
# Run from backend directory
./deploy-all.sh
```

#### 2. Deploy Backend Only (`backend/deploy.sh`)
Updates only the Rust backend.
*   Copies `docker-compose.yml` and `.env.production` to the VPS.
*   Pulls the latest `johnsonf/tradstry-backend` image.
*   Restarts the container.

```bash
# Run from backend directory
make deploy
```

#### 3. Deploy SnapTrade Service (`backend/snaptrade-service/deploy.sh`)
Updates only the Go microservice.
*   Copies its `docker-compose.yml` and `.env` to the VPS.
*   Pulls the latest `johnsonf/snaptrade-service` image.
*   Restarts the container.

```bash
# Run from backend/snaptrade-service directory
make deploy
```

---

## VPS Configuration

The services are deployed to `/opt/tradstry` on the VPS.

### Directory Structure on VPS
```
/opt/tradstry/
├── docker-compose.yml      # Main backend compose file
├── .env                    # Main backend env vars
└── ...                     # SnapTrade service files (managed by its deploy script)
```

### Manual Server Management
If scripts fail, you can manage the server manually via SSH:

```bash
# SSH into server
# Replace the IP address with your VPS IP `root@95.216.219.131`
ssh -i ~/.ssh/id_ed25519_vps root@95.216.219.131

# Navigate to app directory
cd /opt/tradstry

# Check status
docker compose ps

# View logs
docker compose logs -f backend
docker compose logs -f snaptrade-service

# Restart specific service
docker compose restart backend
```

## Release Process

To deploy a new version:

1.  **Commit & Push:** Ensure all code is committed.
2.  **Tag Release:** Create and push a new tag to trigger the CD pipeline.
    ```bash
    git tag -a v1.0.1 -m "Release v1.0.1"
    git push origin v1.0.1
    ```
3.  **Wait for Build:** Check GitHub Actions to ensure images are built and pushed.
4.  **Deploy:** Run the deployment script from your local machine.
    ```bash
    cd backend
    ./deploy-all.sh
    ```
