package handlers

import (
	"errors"
	"snaptrade-service/client"

	"github.com/gofiber/fiber/v2"
)

// respondSnapTradeError forwards SnapTrade's HTTP status and structured
// code/detail when the SDK surfaced an API-level failure, so callers can match
// on `snaptrade_code` instead of parsing free-form strings. Flattening these
// into a generic 500 hides the distinction between "SnapTrade rejected these
// credentials" and "the service is broken", which the Rust client needs in
// order to trigger stale-credential recovery.
func respondSnapTradeError(c *fiber.Ctx, message string, err error) error {
	var apiErr *client.SnapTradeAPIError
	if errors.As(err, &apiErr) {
		return c.Status(apiErr.Status).JSON(fiber.Map{
			"error":            message,
			"snaptrade_status": apiErr.Status,
			"snaptrade_code":   apiErr.Code,
			"snaptrade_detail": apiErr.Detail,
		})
	}
	return c.Status(500).JSON(fiber.Map{
		"error": err.Error(),
	})
}
