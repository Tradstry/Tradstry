package handlers

import (
	"snaptrade-service/client"

	"github.com/gofiber/fiber/v2"
)

// InitiateConnectionRequest represents the request to initiate a connection
type InitiateConnectionRequest struct {
	BrokerageID    string `json:"brokerage_id"`
	ConnectionType string `json:"connection_type,omitempty"` // "read" or "trade", defaults to "read"
	UserSecret     string `json:"user_secret"`               // Passed from Rust backend
}

// InitiateConnectionResponse represents the response from initiating a connection
type InitiateConnectionResponse struct {
	RedirectURL  string `json:"redirect_url"`
	ConnectionID string `json:"connection_id"`
}

// InitiateConnection generates a connection portal URL
func InitiateConnection(snapTradeClient *client.SnapTradeClient) fiber.Handler {
	return func(c *fiber.Ctx) error {
		userId := c.Get("X-User-Id")
		if userId == "" {
			return c.Status(401).JSON(fiber.Map{
				"error": "Missing user ID",
			})
		}

		var req InitiateConnectionRequest
		if err := c.BodyParser(&req); err != nil {
			return c.Status(400).JSON(fiber.Map{
				"error": "Invalid request body",
			})
		}

		if req.BrokerageID == "" {
			return c.Status(400).JSON(fiber.Map{
				"error": "brokerage_id is required",
			})
		}

		if req.UserSecret == "" {
			return c.Status(400).JSON(fiber.Map{
				"error": "user_secret is required",
			})
		}

		connectionType := req.ConnectionType
		if connectionType == "" {
			connectionType = "read"
		}

		redirectToken, err := snapTradeClient.GenerateConnectionPortalURL(
			userId,
			req.UserSecret,
			req.BrokerageID,
			connectionType,
		)
		if err != nil {
			return c.Status(500).JSON(fiber.Map{
				"error": err.Error(),
			})
		}

		// Check if redirect URI exists
		redirectURI, hasRedirectURI := redirectToken.GetRedirectURIOk()
		if !hasRedirectURI || redirectURI == nil || *redirectURI == "" {
			return c.Status(500).JSON(fiber.Map{
				"error": "No redirect URI received from SnapTrade",
			})
		}

		sessionId, hasSessionId := redirectToken.GetSessionIdOk()
		if !hasSessionId || sessionId == nil || *sessionId == "" {
			return c.Status(500).JSON(fiber.Map{
				"error": "No session ID received from SnapTrade",
			})
		}

		return c.JSON(InitiateConnectionResponse{
			RedirectURL:  *redirectURI,
			ConnectionID: *sessionId,
		})
	}
}

// GetConnectionStatus checks the status of a connection
func GetConnectionStatus(snapTradeClient *client.SnapTradeClient) fiber.Handler {
	return func(c *fiber.Ctx) error {
		userId := c.Get("X-User-Id")
		if userId == "" {
			return c.Status(401).JSON(fiber.Map{
				"error": "Missing user ID",
			})
		}

		connectionId := c.Params("connectionId")
		if connectionId == "" {
			return c.Status(400).JSON(fiber.Map{
				"error": "connection_id is required",
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

		status, err := snapTradeClient.GetConnectionStatus(userId, userSecret, connectionId)
		if err != nil {
			return c.Status(500).JSON(fiber.Map{
				"error": err.Error(),
			})
		}

		return c.JSON(status)
	}
}

// ListConnections lists all connections for a user
func ListConnections(snapTradeClient *client.SnapTradeClient) fiber.Handler {
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

		connections, err := snapTradeClient.ListConnections(userId, userSecret)
		if err != nil {
			return c.Status(500).JSON(fiber.Map{
				"error": err.Error(),
			})
		}

		return c.JSON(connections)
	}
}

// DeleteConnection deletes a connection
func DeleteConnection(snapTradeClient *client.SnapTradeClient) fiber.Handler {
	return func(c *fiber.Ctx) error {
		userId := c.Get("X-User-Id")
		if userId == "" {
			return c.Status(401).JSON(fiber.Map{
				"error": "Missing user ID",
			})
		}

		connectionId := c.Params("connectionId")
		if connectionId == "" {
			return c.Status(400).JSON(fiber.Map{
				"error": "connection_id is required",
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

		err := snapTradeClient.DeleteConnection(userId, userSecret, connectionId)
		if err != nil {
			return c.Status(500).JSON(fiber.Map{
				"error": err.Error(),
			})
		}

		return c.JSON(fiber.Map{
			"success": true,
			"message": "Connection deleted successfully",
		})
	}
}
