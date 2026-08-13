package client

import (
	"encoding/json"
	"net/http"
	"net/http/httptest"
	"testing"
)

func TestGetAllAccountPositionsPreservesWebullInstrument(t *testing.T) {
	const consumerKey = "test-consumer-key"
	server := httptest.NewServer(http.HandlerFunc(func(writer http.ResponseWriter, request *http.Request) {
		if request.URL.Path != "/accounts/webull-margin/positions/all" {
			t.Fatalf("unexpected path: %s", request.URL.Path)
		}
		if got := request.Header.Get("Signature"); got != snapTradeSignature(consumerKey, request.URL.Path, request.URL.RawQuery) {
			t.Fatalf("unexpected signature: %q", got)
		}
		if request.URL.Query().Get("clientId") != "test-client" || request.URL.Query().Get("userId") != "user-1" {
			t.Fatalf("missing SnapTrade request identity: %s", request.URL.RawQuery)
		}
		writer.Header().Set("Content-Type", "application/json")
		_, _ = writer.Write([]byte(`{
			"results": [{
				"instrument": {
					"kind": "stock",
					"id": "sec-form",
					"symbol": "FORM",
					"raw_symbol": "FORM",
					"description": "FormFactor Inc.",
					"currency": "USD",
					"figi_instrument": null
				},
				"units": "2",
				"price": "128.39",
				"cost_basis": 128.39,
				"currency": "USD"
			}],
			"data_freshness": {"as_of": "2026-08-13T17:27:43Z"}
		}`))
	}))
	defer server.Close()

	client := &SnapTradeClient{
		httpClient:  server.Client(),
		clientID:    "test-client",
		consumerKey: consumerKey,
		baseURL:     server.URL,
	}
	positions, asOf, _, err := client.getAllAccountPositions("user-1", "secret-1", "webull-margin")
	if err != nil {
		t.Fatalf("get positions: %v", err)
	}
	if len(positions) != 1 {
		t.Fatalf("expected one position, got %d", len(positions))
	}
	position := positions[0]
	if position.Symbol != "FORM" || position.InstrumentID != "sec-form" || position.Units == nil || *position.Units != 2 {
		t.Fatalf("unexpected normalized position: %#v", position)
	}
	if asOf == nil || *asOf != "2026-08-13T17:27:43Z" {
		t.Fatalf("unexpected as-of: %v", asOf)
	}
}

func TestNormalizePositionsRejectsIncompleteInstrument(t *testing.T) {
	var raw allAccountPositionsResponse
	if err := json.Unmarshal([]byte(`{"results":[{"instrument":{"kind":"stock"}}]}`), &raw); err != nil {
		t.Fatalf("decode fixture: %v", err)
	}
	if _, err := normalizePositions(raw); err == nil {
		t.Fatal("expected incomplete instrument to fail normalization")
	}
}
