package main

import (
	"log"
	"os"

	"snaptrade-service/client"
	"snaptrade-service/handlers"

	"github.com/gofiber/fiber/v2"
	"github.com/gofiber/fiber/v2/middleware/cors"
	"github.com/gofiber/fiber/v2/middleware/logger"
	"github.com/joho/godotenv"
)

func main() {
	// Load .env file
	if err := godotenv.Load(); err != nil {
		log.Printf("Warning: Error loading .env file: %v. Will try to use environment variables directly.", err)
	}

	// Initialize SnapTrade client
	snapTradeClient, err := client.NewSnapTradeClient()
	if err != nil {
		log.Fatalf("Failed to initialize SnapTrade client: %v", err)
	}

	// Create Fiber app
	app := fiber.New(fiber.Config{
		ErrorHandler: func(c *fiber.Ctx, err error) error {
			code := fiber.StatusInternalServerError
			if e, ok := err.(*fiber.Error); ok {
				code = e.Code
			}
			return c.Status(code).JSON(fiber.Map{
				"error": err.Error(),
			})
		},
	})

	// Middleware
	app.Use(logger.New())
	app.Use(cors.New(cors.Config{
		AllowOrigins: "*",
		AllowMethods: "GET,POST,PUT,DELETE,OPTIONS",
		AllowHeaders: "Content-Type,Authorization,X-User-Id,X-User-Secret",
	}))

	// Health check
	app.Get("/health", func(c *fiber.Ctx) error {
		return c.JSON(fiber.Map{
			"status":  "ok",
			"service": "snaptrade-service",
		})
	})

	// API routes
	api := app.Group("/api/v1")

	// User management
	api.Post("/users", handlers.CreateSnapTradeUser(snapTradeClient))
	api.Get("/users/:userId", handlers.GetSnapTradeUser(snapTradeClient))
	api.Delete("/users/:userId", handlers.DeleteSnapTradeUser(snapTradeClient))

	// Connection management
	api.Post("/connections/initiate", handlers.InitiateConnection(snapTradeClient))
	api.Get("/connections/:connectionId/status", handlers.GetConnectionStatus(snapTradeClient))
	api.Get("/connections", handlers.ListConnections(snapTradeClient))
	api.Delete("/connections/:connectionId", handlers.DeleteConnection(snapTradeClient))

	// Account management
	api.Get("/accounts", handlers.ListAccounts(snapTradeClient))
	api.Get("/accounts/:accountId", handlers.GetAccountDetail(snapTradeClient))
	api.Post("/accounts/sync", handlers.SyncAccounts(snapTradeClient))

	// Transaction and holdings
	api.Get("/accounts/:accountId/transactions", handlers.GetTransactions(snapTradeClient))
	api.Get("/accounts/:accountId/holdings", handlers.GetHoldings(snapTradeClient))
	api.Get("/accounts/:accountId/holdings/options", handlers.GetOptionPositions(snapTradeClient))

	// Get port from environment or default
	port := os.Getenv("PORT")
	if port == "" {
		port = "8080"
	}

	log.Printf("SnapTrade service starting on 0.0.0.0:%s", port)
	log.Fatal(app.Listen("0.0.0.0:" + port))
}
