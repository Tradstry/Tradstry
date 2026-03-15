package handlers

import (
	"snaptrade-service/client"
	"strings"

	"github.com/gofiber/fiber/v2"
)

// CreateUserRequest represents the request to create a SnapTrade user
type CreateUserRequest struct {
	UserId string `json:"user_id"`
}

// CreateUserResponse represents the response from creating a user
type CreateUserResponse struct {
	UserId     string `json:"user_id"`
	UserSecret string `json:"user_secret"`
}

// CreateSnapTradeUser creates a new SnapTrade user
func CreateSnapTradeUser(snapTradeClient *client.SnapTradeClient) fiber.Handler {
	return func(c *fiber.Ctx) error {
		userId := c.Get("X-User-Id")
		if userId == "" {
			return c.Status(401).JSON(fiber.Map{
				"error": "Missing user ID",
			})
		}

		var req CreateUserRequest
		if err := c.BodyParser(&req); err != nil {
			// If body parsing fails, use the userId from header
			req.UserId = userId
		}

		// Use userId from header if not provided in body
		if req.UserId == "" {
			req.UserId = userId
		}

		result, err := snapTradeClient.CreateUser(req.UserId)
		if err != nil {
			// Check if error is due to user already existing
			errorMsg := err.Error()
			if strings.Contains(errorMsg, "400") || strings.Contains(errorMsg, "already exist") {
				// User might already exist - return structured error response
				return c.Status(400).JSON(fiber.Map{
					"error":   "User already exists in SnapTrade",
					"details": errorMsg,
					"code":    "USER_ALREADY_EXISTS",
					"user_id": req.UserId,
				})
			}
			return c.Status(500).JSON(fiber.Map{
				"error":   "Failed to create SnapTrade user",
				"details": errorMsg,
				"code":    "CREATE_USER_FAILED",
			})
		}

		return c.JSON(CreateUserResponse{
			UserId:     result.GetUserId(),
			UserSecret: result.GetUserSecret(),
		})
	}
}

// GetSnapTradeUser gets information about a SnapTrade user (if needed)
func GetSnapTradeUser(snapTradeClient *client.SnapTradeClient) fiber.Handler {
	return func(c *fiber.Ctx) error {
		userId := c.Get("X-User-Id")
		if userId == "" {
			return c.Status(401).JSON(fiber.Map{
				"error": "Missing user ID",
			})
		}

		// For now, just return the user ID
		// SnapTrade doesn't have a direct "get user" endpoint
		return c.JSON(fiber.Map{
			"user_id": userId,
		})
	}
}

// DeleteUserRequest represents the request to delete a SnapTrade user
type DeleteUserRequest struct {
	UserSecret string `json:"user_secret"`
}

// DeleteSnapTradeUser deletes a SnapTrade user
func DeleteSnapTradeUser(snapTradeClient *client.SnapTradeClient) fiber.Handler {
	return func(c *fiber.Ctx) error {
		userId := c.Get("X-User-Id")
		if userId == "" {
			return c.Status(401).JSON(fiber.Map{
				"error": "Missing user ID",
			})
		}

		// Get user_secret from header, query parameter, or body
		userSecret := c.Get("X-User-Secret")
		if userSecret == "" {
			userSecret = c.Query("user_secret")
		}
		if userSecret == "" {
			var req DeleteUserRequest
			if err := c.BodyParser(&req); err == nil && req.UserSecret != "" {
				userSecret = req.UserSecret
			}
		}
		if userSecret == "" {
			return c.Status(400).JSON(fiber.Map{
				"error": "user_secret is required (header X-User-Secret, query param, or body)",
			})
		}

		err := snapTradeClient.DeleteUser(userId, userSecret)
		if err != nil {
			return c.Status(500).JSON(fiber.Map{
				"error":   "Failed to delete SnapTrade user",
				"details": err.Error(),
			})
		}

		return c.JSON(fiber.Map{
			"success": true,
			"message": "User deleted successfully",
		})
	}
}
