package main

import (
	"errors"
	"log"
	"net"
	"os"
	"os/signal"
	"path/filepath"
	"syscall"
	"time"

	"snaptrade-service/client"
	pb "snaptrade-service/gen/snaptrade/v1"
	adapterrpc "snaptrade-service/rpc"

	"github.com/joho/godotenv"
	"google.golang.org/grpc"
	"google.golang.org/grpc/keepalive"
)

const defaultSocketPath = "/tmp/tradstry-snaptrade.sock"

func main() {
	if err := godotenv.Load(); err != nil {
		log.Printf(".env not loaded; using process environment: %v", err)
	}
	snapTradeClient, err := client.NewSnapTradeClient()
	if err != nil {
		log.Fatalf("initialize SnapTrade client: %v", err)
	}
	server, err := adapterrpc.NewServer(snapTradeClient, os.Getenv("SNAPTRADE_INTERNAL_SECRET"))
	if err != nil {
		log.Fatalf("initialize gRPC adapter: %v", err)
	}
	socketPath := os.Getenv("SNAPTRADE_GRPC_SOCKET")
	if socketPath == "" {
		socketPath = defaultSocketPath
	}
	listener, err := listenUnix(socketPath)
	if err != nil {
		log.Fatalf("listen on private gRPC socket: %v", err)
	}
	defer listener.Close()

	grpcServer := grpc.NewServer(
		grpc.MaxRecvMsgSize(1<<20),
		grpc.MaxSendMsgSize(8<<20),
		grpc.KeepaliveParams(keepalive.ServerParameters{MaxConnectionIdle: 2 * time.Minute}),
	)
	pb.RegisterSnapTradeAdapterServiceServer(grpcServer, server)

	shutdown := make(chan os.Signal, 1)
	signal.Notify(shutdown, syscall.SIGINT, syscall.SIGTERM)
	go func() {
		<-shutdown
		stopped := make(chan struct{})
		go func() {
			grpcServer.GracefulStop()
			close(stopped)
		}()
		select {
		case <-stopped:
		case <-time.After(20 * time.Second):
			grpcServer.Stop()
		}
	}()

	log.Printf("SnapTrade adapter listening on Unix socket %s", socketPath)
	if err := grpcServer.Serve(listener); err != nil && !errors.Is(err, grpc.ErrServerStopped) {
		log.Fatalf("serve gRPC adapter: %v", err)
	}
}

func listenUnix(socketPath string) (net.Listener, error) {
	if !filepath.IsAbs(socketPath) {
		return nil, errors.New("SNAPTRADE_GRPC_SOCKET must be an absolute path")
	}
	if err := os.MkdirAll(filepath.Dir(socketPath), 0o750); err != nil {
		return nil, err
	}
	if info, err := os.Lstat(socketPath); err == nil {
		if info.Mode()&os.ModeSocket == 0 {
			return nil, errors.New("refusing to replace non-socket gRPC path")
		}
		if err := os.Remove(socketPath); err != nil {
			return nil, err
		}
	} else if !errors.Is(err, os.ErrNotExist) {
		return nil, err
	}
	listener, err := net.Listen("unix", socketPath)
	if err != nil {
		return nil, err
	}
	if err := os.Chmod(socketPath, 0o660); err != nil {
		_ = listener.Close()
		return nil, err
	}
	return listener, nil
}
