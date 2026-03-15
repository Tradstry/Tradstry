package handlers

import (
	"snaptrade-service/client"

	"github.com/gofiber/fiber/v2"
)

// ListAccounts lists all accounts for a user
func ListAccounts(snapTradeClient *client.SnapTradeClient) fiber.Handler {
	return func(c *fiber.Ctx) error {
		userId := c.Get("X-User-Id")
		if userId == "" {
			return c.Status(401).JSON(fiber.Map{
				"error": "Missing user ID",
			})
		}

		// Get user_secret from header or query parameter
		userSecret := c.Get("X-User-Secret")
		if userSecret == "" {
			userSecret = c.Query("user_secret")
		}
		if userSecret == "" {
			return c.Status(400).JSON(fiber.Map{
				"error": "user_secret is required (header X-User-Secret or query param)",
			})
		}

		accounts, err := snapTradeClient.ListAccounts(userId, userSecret)
		if err != nil {
			return c.Status(500).JSON(fiber.Map{
				"error": err.Error(),
			})
		}

		return c.JSON(accounts)
	}
}

// GetAccountDetail gets detailed information about a specific account
func GetAccountDetail(snapTradeClient *client.SnapTradeClient) fiber.Handler {
	return func(c *fiber.Ctx) error {
		userId := c.Get("X-User-Id")
		if userId == "" {
			return c.Status(401).JSON(fiber.Map{
				"error": "Missing user ID",
			})
		}

		accountId := c.Params("accountId")
		if accountId == "" {
			return c.Status(400).JSON(fiber.Map{
				"error": "account_id is required",
			})
		}

		// Get user_secret from header or query parameter
		userSecret := c.Get("X-User-Secret")
		if userSecret == "" {
			userSecret = c.Query("user_secret")
		}
		if userSecret == "" {
			return c.Status(400).JSON(fiber.Map{
				"error": "user_secret is required (header X-User-Secret or query param)",
			})
		}

		account, err := snapTradeClient.GetAccountDetail(userId, userSecret, accountId)
		if err != nil {
			return c.Status(500).JSON(fiber.Map{
				"error": err.Error(),
			})
		}

		return c.JSON(account)
	}
}

// SyncAccountsRequest represents the request to sync accounts
type SyncAccountsRequest struct {
	UserSecret string `json:"user_secret"`
}

// SyncAccountsResponse represents the synced data
type SyncAccountsResponse struct {
	Accounts     []interface{} `json:"accounts"`
	Holdings     []interface{} `json:"holdings"`
	Transactions []interface{} `json:"transactions"`
}

// SyncAccounts batch fetches holdings and transactions for all accounts
func SyncAccounts(snapTradeClient *client.SnapTradeClient) fiber.Handler {
	return func(c *fiber.Ctx) error {
		userId := c.Get("X-User-Id")
		if userId == "" {
			return c.Status(401).JSON(fiber.Map{
				"error": "Missing user ID",
			})
		}

		var req SyncAccountsRequest
		if err := c.BodyParser(&req); err != nil {
			return c.Status(400).JSON(fiber.Map{
				"error": "Invalid request body",
			})
		}

		if req.UserSecret == "" {
			return c.Status(400).JSON(fiber.Map{
				"error": "user_secret is required",
			})
		}

		// Get all accounts
		accounts, err := snapTradeClient.ListAccounts(userId, req.UserSecret)
		if err != nil {
			return c.Status(500).JSON(fiber.Map{
				"error": "Failed to list accounts: " + err.Error(),
			})
		}

		var response SyncAccountsResponse
		response.Accounts = make([]interface{}, 0, len(accounts))
		response.Holdings = make([]interface{}, 0)
		response.Transactions = make([]interface{}, 0)

		// Convert accounts to interface{} for JSON serialization
		for _, account := range accounts {
			response.Accounts = append(response.Accounts, account)
		}

		// For each account, fetch holdings and transactions
		for _, account := range accounts {
			accountId := account.GetId()

			// Get holdings
			holdingsAccount, err := snapTradeClient.GetHoldings(userId, req.UserSecret, accountId)
			if err == nil && holdingsAccount != nil {
				// Extract positions from AccountHoldingsAccount
				positions := holdingsAccount.GetPositions()
				for _, position := range positions {
					response.Holdings = append(response.Holdings, position)
				}
			}

			// Get transactions (last 90 days by default, or all if not specified)
			transactions, err := snapTradeClient.GetTransactions(userId, req.UserSecret, accountId, nil, nil)
			if err == nil {
				for _, transaction := range transactions {
					response.Transactions = append(response.Transactions, transaction)
				}
			}
		}

		return c.JSON(response)
	}
}
