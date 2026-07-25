package handlers

import (
	"snaptrade-service/client"
	"strconv"

	"github.com/gofiber/fiber/v2"
)

// GetTransactions fetches transactions for a specific account using the per-account endpoint.
// Supports query params: start_date, end_date, type, offset, limit.
func GetTransactions(snapTradeClient *client.SnapTradeClient) fiber.Handler {
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

		userSecret := c.Get("X-User-Secret")
		if userSecret == "" {
			userSecret = c.Query("user_secret")
		}
		if userSecret == "" {
			return c.Status(400).JSON(fiber.Map{
				"error": "user_secret is required (header X-User-Secret or query param)",
			})
		}

		// Optional query params
		var startDatePtr, endDatePtr, activityTypePtr *string
		if v := c.Query("start_date"); v != "" {
			startDatePtr = &v
		}
		if v := c.Query("end_date"); v != "" {
			endDatePtr = &v
		}
		if v := c.Query("type"); v != "" {
			activityTypePtr = &v
		}

		var offsetPtr, limitPtr *int32
		if v := c.Query("offset"); v != "" {
			if parsed, err := strconv.ParseInt(v, 10, 32); err == nil && parsed >= 0 {
				val := int32(parsed)
				offsetPtr = &val
			}
		}
		if v := c.Query("limit"); v != "" {
			if parsed, err := strconv.ParseInt(v, 10, 32); err == nil && parsed > 0 && parsed <= 1000 {
				val := int32(parsed)
				limitPtr = &val
			}
		}

		result, err := snapTradeClient.GetTransactions(userId, userSecret, accountId, startDatePtr, endDatePtr, activityTypePtr, offsetPtr, limitPtr)
		if err != nil {
			return c.Status(500).JSON(fiber.Map{
				"error": err.Error(),
			})
		}

		return c.JSON(result)
	}
}

// GetHoldings fetches current equity positions for an account
func GetHoldings(snapTradeClient *client.SnapTradeClient) fiber.Handler {
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

		holdings, err := snapTradeClient.GetHoldings(userId, userSecret, accountId)
		if err != nil {
			// Handle empty holdings gracefully
			return c.Status(500).JSON(fiber.Map{
				"error": err.Error(),
			})
		}

		// Filter to only return equity positions (non-option positions)
		if holdings == nil {
			return c.JSON(fiber.Map{
				"positions": []interface{}{},
			})
		}

		positions := holdings.GetPositions()
		equityPositions := make([]interface{}, 0)

		// TODO: Properly filter equity vs options based on SnapTrade SDK structure
		// For now, return all positions as equity (options will be filtered in GetOptionPositions)
		for _, position := range positions {
			// Check if this is clearly an option by examining the position structure
			// This is a simplified approach - may need refinement based on actual data
			isOption := false

			// Try to detect options - this is a placeholder that needs actual SDK inspection
			// Options typically have different structures or type indicators
			if !isOption {
				equityPositions = append(equityPositions, position)
			}
		}

		response := fiber.Map{
			"positions": equityPositions,
		}

		// Include total market value from account.balance.total if present.
		// Chain: AccountHoldingsAccount.GetAccount() → Account.GetBalance() →
		//        AccountBalance.GetTotalOk() → AccountBalanceTotal.{GetAmount, GetCurrency}
		if holdings.HasAccount() {
			acct := holdings.GetAccount()
			balance := acct.GetBalance()
			if total, ok := balance.GetTotalOk(); ok && total != nil {
				tv := fiber.Map{}
				if amt, ok := total.GetAmountOk(); ok {
					tv["amount"] = *amt
				}
				if cur, ok := total.GetCurrencyOk(); ok {
					tv["currency"] = *cur
				}
				if len(tv) > 0 {
					response["total_value"] = tv
				}
			}
		}

		// Include per-currency balances if present (bonus, non-breaking).
		if balances := holdings.GetBalances(); len(balances) > 0 {
			response["balances"] = balances
		}

		// Orders arrive in this same payload and cost no extra request. They are
		// the only intraday signal available: the activities endpoint SnapTrade
		// asks us not to poll more than daily is what confirms a fill, so without
		// these a trade stays invisible until the next day.
		if orders := holdings.GetOrders(); len(orders) > 0 {
			response["orders"] = orders
		}

		return c.JSON(response)
	}
}

// GetOptionPositions fetches current option positions for an account
func GetOptionPositions(snapTradeClient *client.SnapTradeClient) fiber.Handler {
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

		holdings, err := snapTradeClient.GetHoldings(userId, userSecret, accountId)
		if err != nil {
			// Handle empty holdings gracefully - return empty array
			return c.JSON(fiber.Map{
				"positions": []interface{}{},
			})
		}

		// Filter to only return option positions
		if holdings == nil {
			return c.JSON(fiber.Map{
				"positions": []interface{}{},
			})
		}

		positions := holdings.GetPositions()
		optionPositions := make([]interface{}, 0)

		// TODO: Properly filter options based on SnapTrade SDK structure
		// For now, return empty array - this needs to be implemented based on actual position structure
		// Options may have a type field, symbol format, or other indicators
		for _, position := range positions {
			// Check if this is an option position
			// This is a placeholder - needs actual implementation based on SDK structure
			isOption := false

			// Try to detect options - check position type, symbol format, etc.
			// This needs to be implemented based on actual SnapTrade position structure
			if isOption {
				optionPositions = append(optionPositions, position)
			}
		}

		return c.JSON(fiber.Map{
			"positions": optionPositions,
		})
	}
}

// isOptionPosition checks if a symbol represents an option
// Options typically have formats like "AAPL230120C00150000" or contain specific patterns
func isOptionPosition(symbol string) bool {
	if symbol == "" {
		return false
	}
	// Check for option-like patterns:
	// - Contains "C" or "P" followed by numbers (call/put indicators)
	// - Very long symbol strings (options are typically longer)
	// - Contains date-like patterns
	// This is a heuristic - adjust based on actual SnapTrade data
	if len(symbol) > 15 {
		// Options are typically longer than regular stock symbols
		return true
	}
	// Check for call/put indicators in the symbol
	// This is a simplified check - may need refinement
	return false
}
