.PHONY: build run stop logs clean test health

# Build the Docker image
build:
	docker build -t snaptrade-service:latest .

# Run the service using docker-compose
run:
	docker-compose up -d

# Stop the service
stop:
	docker-compose down

# View logs
logs:
	docker-compose logs -f

# Clean up containers and images
clean:
	docker-compose down -v
	docker rmi snaptrade-service:latest || true

# Test the health endpoint
health:
	curl http://localhost:8080/health

# Run tests (if you add tests later)
test:
	go test ./...

# Build and run in one command
build-run: build run

# Restart the service
restart: stop run

