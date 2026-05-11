package client

import (
	"encoding/json"
	"fmt"
)

// SnapTradeAPIError is the structured error returned by client methods when
// SnapTrade itself responds with a non-success status. It preserves the raw
// HTTP status, raw body, and the parsed fields from SnapTrade's standard
// error envelope so handlers can return matching HTTP responses (e.g. 401
// with code 1083 for stale credentials) instead of opaque 500s.
//
// Handlers detect this via errors.As(err, &apiErr).
type SnapTradeAPIError struct {
	// Status is the upstream SnapTrade HTTP status (401, 403, 429, etc.).
	Status int
	// Body is the raw response body bytes from SnapTrade.
	Body []byte
	// Code is SnapTrade's error code string (e.g. "1083"). Empty if the body
	// didn't parse or the field was absent.
	Code string
	// Detail is SnapTrade's human-readable error detail. Empty if absent.
	Detail string
	// Wrapped is the underlying SDK error (used by errors.Is / Unwrap).
	Wrapped error
}

func (e *SnapTradeAPIError) Error() string {
	if e.Code != "" {
		return fmt.Sprintf("SnapTrade API error (status %d, code %s): %s", e.Status, e.Code, e.Detail)
	}
	return fmt.Sprintf("SnapTrade API error (status %d): %s", e.Status, string(e.Body))
}

func (e *SnapTradeAPIError) Unwrap() error {
	return e.Wrapped
}

// NewSnapTradeAPIError parses a SnapTrade response body into structured fields.
// SnapTrade's standard error body shape:
//
//	{ "detail": "...", "status_code": <int>, "code": "<string>" }
func NewSnapTradeAPIError(status int, body []byte, wrapped error) *SnapTradeAPIError {
	e := &SnapTradeAPIError{Status: status, Body: body, Wrapped: wrapped}
	var parsed struct {
		Detail string      `json:"detail"`
		Code   interface{} `json:"code"`
	}
	if err := json.Unmarshal(body, &parsed); err == nil {
		e.Detail = parsed.Detail
		switch v := parsed.Code.(type) {
		case string:
			e.Code = v
		case float64:
			e.Code = fmt.Sprintf("%.0f", v)
		case nil:
			e.Code = ""
		default:
			e.Code = fmt.Sprintf("%v", v)
		}
	}
	return e
}
