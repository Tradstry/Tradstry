package rpc

import (
	"crypto/hmac"
	"crypto/sha256"
	"encoding/hex"
	"fmt"
	"strconv"
	"sync"
	"time"

	pb "snaptrade-service/gen/snaptrade/v1"
)

const maxRequestAge = 5 * time.Minute

type authenticator struct {
	secret []byte
	mu     sync.Mutex
	seen   map[string]time.Time
	now    func() time.Time
}

func newAuthenticator(secret string) (*authenticator, error) {
	if len(secret) < 32 {
		return nil, fmt.Errorf("SNAPTRADE_INTERNAL_SECRET must contain at least 32 bytes")
	}
	return &authenticator{
		secret: []byte(secret),
		seen:   make(map[string]time.Time),
		now:    time.Now,
	}, nil
}

func (a *authenticator) verify(method string, auth *pb.RequestAuth, payload []byte) error {
	if auth == nil || auth.Nonce == "" || auth.Signature == "" {
		return fmt.Errorf("missing internal authentication")
	}
	now := a.now()
	sentAt := time.Unix(auth.UnixSeconds, 0)
	if skew := now.Sub(sentAt); skew > maxRequestAge || skew < -maxRequestAge {
		return fmt.Errorf("expired internal authentication")
	}
	if len(auth.Nonce) > 128 {
		return fmt.Errorf("invalid internal authentication nonce")
	}
	payloadHash := sha256.Sum256(payload)
	message := strconv.FormatInt(auth.UnixSeconds, 10) + "\n" + method + "\n" + auth.Nonce + "\n" + hex.EncodeToString(payloadHash[:])
	mac := hmac.New(sha256.New, a.secret)
	_, _ = mac.Write([]byte(message))
	expected := hex.EncodeToString(mac.Sum(nil))
	if !hmac.Equal([]byte(expected), []byte(auth.Signature)) {
		return fmt.Errorf("invalid internal authentication")
	}

	a.mu.Lock()
	defer a.mu.Unlock()
	for nonce, seenAt := range a.seen {
		if now.Sub(seenAt) > maxRequestAge {
			delete(a.seen, nonce)
		}
	}
	if _, exists := a.seen[auth.Nonce]; exists {
		return fmt.Errorf("replayed internal request")
	}
	a.seen[auth.Nonce] = now
	return nil
}
