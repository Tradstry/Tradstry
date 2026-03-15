package handlers

import (
	"snaptrade-service/client"
	"strconv"

	"github.com/gofiber/fiber/v2"
)

// GetTransactionsRequest represents the request to get transactions
type GetTransactionsRequest struct {
	UserSecret string  `json:"user_secret"`
	StartDate  *string `json:"start_date,omitempty"`
	EndDate    *string `json:"end_date,omitempty"`
	Offset     *int    `json:"offset,omitempty"`
	Limit      *int    `json:"limit,omitempty"`
}

// GetTransactionsResponse represents the paginated response
type GetTransactionsResponse struct {
	Data       []interface{} `json:"data"`
	Pagination struct {
		Offset int `json:"offset"`
		Limit  int `json:"limit"`
		Total  int `json:"total"`
	} `json:"pagination"`
}

// GetTransactions fetches transactions for an account with pagination support
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

		// Parse query parameters
		startDate := c.Query("start_date")
		endDate := c.Query("end_date")
		offsetStr := c.Query("offset")
		limitStr := c.Query("limit")

		var startDatePtr *string
		var endDatePtr *string
		if startDate != "" {
			startDatePtr = &startDate
		}
		if endDate != "" {
			endDatePtr = &endDate
		}

		// Default pagination values
		offset := 0
		limit := 1000 // SnapTrade default

		if offsetStr != "" {
			if parsedOffset, err := strconv.Atoi(offsetStr); err == nil && parsedOffset >= 0 {
				offset = parsedOffset
			}
		}

		if limitStr != "" {
			if parsedLimit, err := strconv.Atoi(limitStr); err == nil && parsedLimit > 0 && parsedLimit <= 1000 {
				limit = parsedLimit
			}
		}

		transactions, err := snapTradeClient.GetTransactions(userId, userSecret, accountId, startDatePtr, endDatePtr)
		if err != nil {
			// Handle empty transactions gracefully - return empty array
			return c.JSON(GetTransactionsResponse{
				Data: []interface{}{},
				Pagination: struct {
					Offset int `json:"offset"`
					Limit  int `json:"limit"`
					Total  int `json:"total"`
				}{
					Offset: offset,
					Limit:  limit,
					Total:  0,
				},
			})
		}

		// Apply pagination
		total := len(transactions)
		start := offset
		end := offset + limit
		if start > total {
			start = total
		}
		if end > total {
			end = total
		}

		var paginatedData []interface{}
		if start < end {
			for i := start; i < end; i++ {
				paginatedData = append(paginatedData, transactions[i])
			}
		}

		response := GetTransactionsResponse{
			Data: paginatedData,
			Pagination: struct {
				Offset int `json:"offset"`
				Limit  int `json:"limit"`
				Total  int `json:"total"`
			}{
				Offset: offset,
				Limit:  limit,
				Total:  total,
			},
		}

		return c.JSON(response)
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

		return c.JSON(fiber.Map{
			"positions": equityPositions,
		})
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
