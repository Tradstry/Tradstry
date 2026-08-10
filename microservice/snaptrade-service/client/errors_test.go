package client

import (
	"errors"
	"net/http"
	"testing"
)

type sdkBodyError struct{ body []byte }

func (errorValue *sdkBodyError) Error() string { return "sdk request failed" }
func (errorValue *sdkBodyError) Body() []byte  { return errorValue.body }

func TestResponseErrorUsesSDKBodyAfterResponseWasConsumed(t *testing.T) {
	response := &http.Response{StatusCode: http.StatusBadRequest, Header: http.Header{}}
	err := responseError(response, &sdkBodyError{body: []byte(`{"code":"1083","detail":"invalid"}`)})
	var apiError *SnapTradeAPIError
	if !errors.As(err, &apiError) {
		t.Fatalf("expected SnapTradeAPIError, got %T", err)
	}
	if apiError.Code != "1083" {
		t.Fatalf("expected code 1083, got %q", apiError.Code)
	}
}
