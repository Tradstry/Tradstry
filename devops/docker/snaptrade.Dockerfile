# Build stage
FROM golang:1.26.5-alpine AS builder

# Install build dependencies
RUN apk add --no-cache git ca-certificates tzdata

# Set working directory
WORKDIR /build

# Copy go mod files
COPY go.mod go.sum ./

# Download dependencies
RUN go mod download

# Copy source code
COPY . .

# Build the application
RUN CGO_ENABLED=0 GOOS=linux GOARCH=amd64 go build \
    -ldflags='-w -s -extldflags "-static"' \
    -a -installsuffix cgo \
    -o snaptrade-service \
    .

# Final stage
FROM alpine:3.24.1

# Install certificates for outbound SnapTrade HTTPS calls.
RUN apk --no-cache add ca-certificates tzdata

# Create non-root user
RUN addgroup -g 1000 appuser && \
    adduser -D -u 1000 -G appuser appuser

WORKDIR /app

# Copy binary from builder
COPY --from=builder /build/snaptrade-service .

# Change ownership
RUN mkdir -p /run/tradstry && chown -R appuser:appuser /app /run/tradstry

# Switch to non-root user
USER appuser

# The adapter exposes no TCP port. Rust reaches this Unix socket through a
# volume mounted only into the two application containers.
HEALTHCHECK --interval=30s --timeout=3s --start-period=5s --retries=3 \
    CMD test -S /run/tradstry/snaptrade.sock && kill -0 1 || exit 1

# Run the service
CMD ["./snaptrade-service"]
