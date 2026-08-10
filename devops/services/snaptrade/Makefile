.PHONY: build run stop logs clean test health

ROOT := $(abspath $(dir $(lastword $(MAKEFILE_LIST)))/../../..)
COMPOSE := docker compose --env-file $(ROOT)/devops/.env -f $(ROOT)/devops/compose.yml

# Build the Docker image
build:
	docker build -f $(ROOT)/devops/docker/snaptrade.Dockerfile \
		-t snaptrade-service:latest $(ROOT)/microservice/snaptrade-service

# Run the service using the repository Compose stack
run:
	$(COMPOSE) up -d snaptrade-service

# Stop the service
stop:
	$(COMPOSE) stop snaptrade-service

# View logs
logs:
	$(COMPOSE) logs -f snaptrade-service

# Clean up containers and images
clean:
	$(COMPOSE) rm -sfv snaptrade-service
	docker rmi snaptrade-service:latest || true

# Confirm the private gRPC socket exists.
health:
	test -S /tmp/tradstry-snaptrade.sock

# Run tests (if you add tests later)
test:
	cd $(ROOT)/microservice/snaptrade-service && go test ./...

# Build and run in one command
build-run: build run

# Restart the service
restart: stop run
