# SnapTrade adapter

The Go 1.26.5 service is the authoritative adapter between SnapTrade and the Rust
backend. It converts upstream SDK models into the versioned protobuf contract
at `backend/proto/tradstry/snaptrade/v1/adapter.proto`.

It does not listen on TCP and must never be exposed through Caddy or a published
Docker port. Rust reaches it with gRPC over a Unix-domain socket mounted only
into the backend and adapter containers. Every request also includes a
timestamped, nonce-protected HMAC.

Public SnapTrade webhooks go directly to the Rust backend at
`POST /webhooks/snaptrade`. Rust verifies the `Signature` header with
`SNAPTRADE_CONSUMER_KEY`, rejects stale events, stores each event durably, and
runs targeted reconciliation in its retry worker.

## Configuration

Create `microservice/snaptrade-service/.env`:

```bash
SNAPTRADE_CLIENT_ID=your_client_id
SNAPTRADE_CONSUMER_KEY=your_consumer_key
SNAPTRADE_INTERNAL_SECRET=generate_at_least_32_random_bytes
SNAPTRADE_GRPC_SOCKET=/tmp/tradstry-snaptrade.sock
```

The Rust backend uses the same socket, internal secret, and consumer key.
Production Compose changes the socket to `/run/tradstry/snaptrade.sock`.

## Local development

Start Go first so it creates the socket, then start Rust:

```bash
make micro
cd backend && cargo run
```

Run adapter checks:

```bash
cd microservice/snaptrade-service
go test ./...
go vet ./...
```

## Protobuf generation

Generated Go code is checked in so production images do not need code-generation
tools. Regenerate it from the repository root with the pinned Buf plugins:

```bash
go run github.com/bufbuild/buf/cmd/buf@v1.72.0 lint
go run github.com/bufbuild/buf/cmd/buf@v1.72.0 generate
```

Rust code is generated during the Cargo build with a vendored `protoc` binary,
so developer machines do not need a global protobuf compiler.

## Production

```bash
docker compose --env-file devops/.env -f devops/compose.yml up -d --build snaptrade-service backend
```

The adapter health check confirms the Unix socket exists and the process is
alive. No adapter port should appear in `docker compose ps`.
