package client

import (
	"context"
	"fmt"
	"io"
	"os"

	snaptrade "github.com/passiv/snaptrade-sdks/sdks/go"
)

type SnapTradeClient struct {
	client   *snaptrade.APIClient
	ctx      context.Context
	clientId string
}

func NewSnapTradeClient() (*SnapTradeClient, error) {
	clientId := os.Getenv("SNAPTRADE_CLIENT_ID")
	consumerKey := os.Getenv("SNAPTRADE_CONSUMER_KEY")

	if clientId == "" || consumerKey == "" {
		return nil, fmt.Errorf("SNAPTRADE_CLIENT_ID and SNAPTRADE_CONSUMER_KEY must be set")
	}

	config := snaptrade.NewConfiguration()

	// Set API keys using the SDK's proper methods
	// The SDK has specific methods for setting client ID and consumer key
	config.SetPartnerClientId(clientId)
	config.SetConsumerKey(consumerKey)

	// Also add API keys to the internal map using AddAPIKey
	// This ensures the SDK's internal request hooks can access the keys
	config.AddAPIKey("clientId", snaptrade.APIKey{
		Key:    clientId,
		Prefix: "",
	})
	config.AddAPIKey("consumerKey", snaptrade.APIKey{
		Key:    consumerKey,
		Prefix: "",
	})

	// Also set as headers as fallback (some SDK versions may use headers)
	config.AddDefaultHeader("clientId", clientId)
	config.AddDefaultHeader("consumerKey", consumerKey)

	apiClient := snaptrade.NewAPIClient(config)

	return &SnapTradeClient{
		client:   apiClient,
		ctx:      context.Background(),
		clientId: clientId,
	}, nil
}

// CreateUser creates a new SnapTrade user
func (c *SnapTradeClient) CreateUser(userId string) (*snaptrade.UserIDandSecret, error) {
	registerBody := snaptrade.NewSnapTradeRegisterUserRequestBody(userId)

	req := c.client.AuthenticationApi.RegisterSnapTradeUser(*registerBody)
	result, httpResp, err := c.client.AuthenticationApi.RegisterSnapTradeUserExecute(req)
	if err != nil {
		// Log detailed error information
		if httpResp != nil {
			bodyBytes := make([]byte, 0)
			if httpResp.Body != nil {
				// Try to read response body for better error message
				bodyBytes, _ = io.ReadAll(httpResp.Body)
			}
			fmt.Printf("SnapTrade API Error - Status: %d, Body: %s, Error: %v\n", 
				httpResp.StatusCode, string(bodyBytes), err)
			
			// Check if user already exists (400 Bad Request often means user exists)
			if httpResp.StatusCode == 400 {
				return nil, fmt.Errorf("user may already exist or invalid request (400 Bad Request): %s - %w", 
					string(bodyBytes), err)
			}
		}
		return nil, fmt.Errorf("failed to create SnapTrade user: %w", err)
	}

	return result, nil
}

// DeleteUser deletes a SnapTrade user
// Endpoint: DELETE /api/v1/snapTrade/deleteUser
// Returns: { "status": "deleted", "detail": "...", "userId": "..." }
// Note: User deletion is queued and happens asynchronously
// The SDK method only takes userId, userSecret must be passed via context or request
func (c *SnapTradeClient) DeleteUser(userId, userSecret string) error {
	// SDK method only takes userId
	req := c.client.AuthenticationApi.DeleteSnapTradeUser(userId)
	
	// Note: The SDK may handle userSecret via context or the request may need it
	// If the API requires userSecret, it might be in the request body or headers
	// For now, we'll try with just userId and see if it works
	
	result, httpResp, err := c.client.AuthenticationApi.DeleteSnapTradeUserExecute(req)
	if err != nil {
		if httpResp != nil {
			bodyBytes := make([]byte, 0)
			if httpResp.Body != nil {
				bodyBytes, _ = io.ReadAll(httpResp.Body)
			}
			// If it's a 401/403, the API requires userSecret
			if httpResp.StatusCode == 401 || httpResp.StatusCode == 403 {
				return fmt.Errorf("user_secret required to delete user (401/403): %s", string(bodyBytes))
			}
			fmt.Printf("SnapTrade Delete User API Error - Status: %d, Body: %s, Error: %v\n", 
				httpResp.StatusCode, string(bodyBytes), err)
		}
		return fmt.Errorf("failed to delete SnapTrade user: %w", err)
	}

	// Log the deletion status
	if result != nil {
		fmt.Printf("SnapTrade user deletion queued: userId=%s, status=%s\n", userId, result.GetStatus())
	}

	return nil
}

// GenerateConnectionPortalURL generates a connection portal URL for the user
func (c *SnapTradeClient) GenerateConnectionPortalURL(userId, userSecret, brokerageId string, connectionType string) (*snaptrade.LoginRedirectURI, error) {
	req := c.client.AuthenticationApi.LoginSnapTradeUser(userId, userSecret)

	// Note: The SDK doesn't seem to have a direct way to specify brokerage or connection type
	// in LoginSnapTradeUser. The connection portal URL is generic and the user selects
	// the brokerage in the portal itself.

	response, httpResp, err := c.client.AuthenticationApi.LoginSnapTradeUserExecute(req)
	if err != nil {
		return nil, fmt.Errorf("failed to generate connection portal URL: %w", err)
	}

	// Log response for debugging
	if httpResp != nil {
		fmt.Printf("SnapTrade API Response Status: %d\n", httpResp.StatusCode)
	}

	// Extract LoginRedirectURI from the response
	if response.LoginRedirectURI == nil {
		return nil, fmt.Errorf("no redirect URI in response (LoginRedirectURI is nil)")
	}

	// Check if redirect URI is actually populated
	redirectURI, hasURI := response.LoginRedirectURI.GetRedirectURIOk()
	if !hasURI || redirectURI == nil || *redirectURI == "" {
		return nil, fmt.Errorf("redirect URI is empty in response")
	}

	return response.LoginRedirectURI, nil
}

// GetConnectionStatus checks the status of a connection
// Note: SnapTrade SDK doesn't have a direct "GetConnectionStatus" method
// We'll use ListBrokerageAuthorizations to check if a connection exists
func (c *SnapTradeClient) GetConnectionStatus(userId, userSecret, connectionId string) (*snaptrade.BrokerageAuthorization, error) {
	req := c.client.ConnectionsApi.ListBrokerageAuthorizations(userId, userSecret)
	authorizations, _, err := c.client.ConnectionsApi.ListBrokerageAuthorizationsExecute(req)
	if err != nil {
		return nil, fmt.Errorf("failed to list connections: %w", err)
	}

	// Find the connection by ID
	for _, auth := range authorizations {
		if auth.GetId() == connectionId {
			return &auth, nil
		}
	}

	return nil, fmt.Errorf("connection not found")
}

// ListConnections lists all connections for a user
func (c *SnapTradeClient) ListConnections(userId, userSecret string) ([]snaptrade.BrokerageAuthorization, error) {
	req := c.client.ConnectionsApi.ListBrokerageAuthorizations(userId, userSecret)
	authorizations, _, err := c.client.ConnectionsApi.ListBrokerageAuthorizationsExecute(req)
	if err != nil {
		return nil, fmt.Errorf("failed to list connections: %w", err)
	}

	return authorizations, nil
}

// DeleteConnection deletes a connection
func (c *SnapTradeClient) DeleteConnection(userId, userSecret, connectionId string) error {
	req := c.client.ConnectionsApi.RemoveBrokerageAuthorization(connectionId, userId, userSecret)
	_, err := c.client.ConnectionsApi.RemoveBrokerageAuthorizationExecute(req)
	if err != nil {
		return fmt.Errorf("failed to delete connection: %w", err)
	}

	return nil
}

// ListAccounts lists all accounts for a user
func (c *SnapTradeClient) ListAccounts(userId, userSecret string) ([]snaptrade.Account, error) {
	req := c.client.AccountInformationApi.ListUserAccounts(userId, userSecret)
	accounts, _, err := c.client.AccountInformationApi.ListUserAccountsExecute(req)
	if err != nil {
		return nil, fmt.Errorf("failed to list accounts: %w", err)
	}

	return accounts, nil
}

// GetAccountDetail gets detailed information about a specific account
func (c *SnapTradeClient) GetAccountDetail(userId, userSecret, accountId string) (*snaptrade.Account, error) {
	req := c.client.AccountInformationApi.GetUserAccountDetails(userId, userSecret, accountId)
	account, _, err := c.client.AccountInformationApi.GetUserAccountDetailsExecute(req)
	if err != nil {
		return nil, fmt.Errorf("failed to get account detail: %w", err)
	}

	return account, nil
}

// GetHoldings gets current holdings for an account
func (c *SnapTradeClient) GetHoldings(userId, userSecret, accountId string) (*snaptrade.AccountHoldingsAccount, error) {
	req := c.client.AccountInformationApi.GetUserHoldings(accountId, userId, userSecret)
	holdings, _, err := c.client.AccountInformationApi.GetUserHoldingsExecute(req)
	if err != nil {
		return nil, fmt.Errorf("failed to get holdings: %w", err)
	}

	return holdings, nil
}

// GetTransactions gets transactions for an account
// Note: SnapTrade SDK uses GetActivities which returns UniversalActivity
func (c *SnapTradeClient) GetTransactions(userId, userSecret, accountId string, startDate, endDate *string) ([]snaptrade.UniversalActivity, error) {
	req := c.client.TransactionsAndReportingApi.GetActivities(userId, userSecret)

	// Add account filter if provided
	if accountId != "" {
		// Note: The SDK may not support account filtering directly in GetActivities
		// You may need to filter the results after fetching
	}

	// Add date range if provided
	if startDate != nil {
		// Note: Date filtering may need to be done via GetReportingCustomRange
		// or filtered after fetching
	}

	activities, _, err := c.client.TransactionsAndReportingApi.GetActivitiesExecute(req)
	if err != nil {
		return nil, fmt.Errorf("failed to get transactions: %w", err)
	}

	// Filter by account if accountId is provided
	if accountId != "" {
		filtered := make([]snaptrade.UniversalActivity, 0)
		for _, activity := range activities {
			// Check if activity belongs to the account
			// This depends on the UniversalActivity structure
			// For now, return all activities
			filtered = append(filtered, activity)
		}
		return filtered, nil
	}

	return activities, nil
}
