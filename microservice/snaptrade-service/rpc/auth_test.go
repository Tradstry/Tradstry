package rpc

import (
	"crypto/hmac"
	"crypto/sha256"
	"encoding/hex"
	"strconv"
	"testing"
	"time"

	pb "snaptrade-service/gen/snaptrade/v1"

	"google.golang.org/protobuf/proto"
)

func signedAuth(t *testing.T, secret, method, nonce string, unixSeconds int64, message proto.Message) (*pb.RequestAuth, []byte) {
	t.Helper()
	payload, err := proto.MarshalOptions{Deterministic: true}.Marshal(message)
	if err != nil {
		t.Fatal(err)
	}
	digest := sha256.Sum256(payload)
	content := strconv.FormatInt(unixSeconds, 10) + "\n" + method + "\n" + nonce + "\n" + hex.EncodeToString(digest[:])
	mac := hmac.New(sha256.New, []byte(secret))
	_, _ = mac.Write([]byte(content))
	return &pb.RequestAuth{
		UnixSeconds: unixSeconds,
		Nonce:       nonce,
		Signature:   hex.EncodeToString(mac.Sum(nil)),
	}, payload
}

func TestAuthenticatorRejectsReplayAndTampering(t *testing.T) {
	const secret = "test-only-secret-that-is-at-least-32-bytes"
	const method = pb.SnapTradeAdapterService_RegisterUser_FullMethodName
	now := time.Unix(1_800_000_000, 0)
	authenticator, err := newAuthenticator(secret)
	if err != nil {
		t.Fatal(err)
	}
	authenticator.now = func() time.Time { return now }
	request := &pb.RegisterUserRequest{UserId: "user-1"}
	auth, payload := signedAuth(t, secret, method, "nonce-1", now.Unix(), request)
	if err := authenticator.verify(method, auth, payload); err != nil {
		t.Fatal(err)
	}
	if err := authenticator.verify(method, auth, payload); err == nil {
		t.Fatal("expected replay to be rejected")
	}

	auth, _ = signedAuth(t, secret, method, "nonce-2", now.Unix(), request)
	tampered, err := proto.MarshalOptions{Deterministic: true}.Marshal(&pb.RegisterUserRequest{UserId: "user-2"})
	if err != nil {
		t.Fatal(err)
	}
	if err := authenticator.verify(method, auth, tampered); err == nil {
		t.Fatal("expected tampered payload to be rejected")
	}
}

func TestAuthenticatorRejectsExpiredRequest(t *testing.T) {
	const secret = "test-only-secret-that-is-at-least-32-bytes"
	now := time.Unix(1_800_000_000, 0)
	authenticator, err := newAuthenticator(secret)
	if err != nil {
		t.Fatal(err)
	}
	authenticator.now = func() time.Time { return now }
	request := &pb.RegisterUserRequest{UserId: "user-1"}
	auth, payload := signedAuth(
		t, secret, pb.SnapTradeAdapterService_RegisterUser_FullMethodName,
		"nonce-1", now.Add(-maxRequestAge-time.Second).Unix(), request,
	)
	if err := authenticator.verify(pb.SnapTradeAdapterService_RegisterUser_FullMethodName, auth, payload); err == nil {
		t.Fatal("expected expired request to be rejected")
	}
}

func TestAuthenticationMatchesRustVector(t *testing.T) {
	const secret = "test-only-secret-that-is-at-least-32-bytes"
	request := &pb.RegisterUserRequest{UserId: "user-1"}
	auth, payload := signedAuth(
		t, secret, pb.SnapTradeAdapterService_RegisterUser_FullMethodName,
		"fixed-nonce", 1_800_000_000, request,
	)
	if got := hex.EncodeToString(payload); got != "1206757365722d31" {
		t.Fatalf("unexpected protobuf payload: %s", got)
	}
	if auth.Signature != "ff913ae344665be19c6fd7de649a8420ea4f0bb5264d4cf7fff9703ab8237a86" {
		t.Fatalf("unexpected signature: %s", auth.Signature)
	}
}
