package client

import (
	"encoding/json"
	"fmt"
	"net/http"
	"strconv"

	"snaptrade-service/contract"
)

// SnapTradeAPIError is the structured error returned by client methods when
// SnapTrade itself responds with a non-success status. It preserves the HTTP
// status and stable code from SnapTrade's standard
// error envelope so handlers can return a stable internal error contract
// without exposing heterogeneous upstream response bodies.
//
// Handlers detect this via errors.As(err, &apiErr).
type SnapTradeAPIError struct {
	// Status is the upstream SnapTrade HTTP status (401, 403, 429, etc.).
	Status int
	// Code is SnapTrade's error code string (e.g. "1083"). Empty if the body
	// didn't parse or the field was absent.
	Code string
	// Wrapped is the underlying SDK error (used by errors.Is / Unwrap).
	Wrapped error
	Meta    contract.ResponseMeta
}

func (e *SnapTradeAPIError) Error() string {
	if e.Code != "" {
		return fmt.Sprintf("SnapTrade API error (status %d, code %s)", e.Status, e.Code)
	}
	return fmt.Sprintf("SnapTrade API error (status %d)", e.Status)
}

func (e *SnapTradeAPIError) Unwrap() error {
	return e.Wrapped
}

// NewSnapTradeAPIError parses a SnapTrade response body into structured fields.
// SnapTrade's standard error body shape:
//
//	{ "detail": "...", "status_code": <int>, "code": "<string>" }
func NewSnapTradeAPIError(response *http.Response, body []byte, wrapped error) *SnapTradeAPIError {
	status := http.StatusBadGateway
	if response != nil {
		status = response.StatusCode
	}
	e := &SnapTradeAPIError{Status: status, Wrapped: wrapped, Meta: ResponseMeta(response)}
	var parsed struct {
		Detail string      `json:"detail"`
		Code   interface{} `json:"code"`
	}
	if err := json.Unmarshal(body, &parsed); err == nil {
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

func ResponseMeta(response *http.Response) contract.ResponseMeta {
	if response == nil {
		return contract.ResponseMeta{}
	}
	meta := contract.ResponseMeta{RequestID: response.Header.Get("X-Request-ID")}
	rate := contract.RateLimit{
		Limit:            parseIntHeader(response, "X-RateLimit-Limit"),
		Remaining:        parseIntHeader(response, "X-RateLimit-Remaining"),
		ResetSeconds:     parseIntHeader(response, "X-RateLimit-Reset"),
		AccountLimit:     parseIntHeader(response, "X-RateLimit-Account-Limit"),
		AccountRemaining: parseIntHeader(response, "X-RateLimit-Account-Remaining"),
		AccountReset:     parseIntHeader(response, "X-RateLimit-Account-Reset"),
	}
	if rate.Limit != nil || rate.Remaining != nil || rate.ResetSeconds != nil ||
		rate.AccountLimit != nil || rate.AccountRemaining != nil || rate.AccountReset != nil {
		meta.RateLimit = &rate
	}
	return meta
}

func parseIntHeader(response *http.Response, name string) *int {
	value := response.Header.Get(name)
	if value == "" {
		return nil
	}
	parsed, err := strconv.Atoi(value)
	if err != nil {
		return nil
	}
	return &parsed
}
