package client

import (
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"net"
	"net/http"
	"os"
	"time"

	"snaptrade-service/contract"

	snaptrade "github.com/passiv/snaptrade-sdks/sdks/go"
)

type SnapTradeClient struct {
	client *snaptrade.APIClient
}

func NewSnapTradeClient() (*SnapTradeClient, error) {
	clientID := os.Getenv("SNAPTRADE_CLIENT_ID")
	consumerKey := os.Getenv("SNAPTRADE_CONSUMER_KEY")
	if clientID == "" || consumerKey == "" {
		return nil, fmt.Errorf("SNAPTRADE_CLIENT_ID and SNAPTRADE_CONSUMER_KEY must be set")
	}

	transport := http.DefaultTransport.(*http.Transport).Clone()
	transport.DialContext = (&net.Dialer{Timeout: 10 * time.Second, KeepAlive: 30 * time.Second}).DialContext
	transport.MaxIdleConns = 100
	transport.MaxIdleConnsPerHost = 20
	transport.IdleConnTimeout = 90 * time.Second
	transport.TLSHandshakeTimeout = 10 * time.Second
	transport.ResponseHeaderTimeout = 20 * time.Second

	config := snaptrade.NewConfiguration()
	config.HTTPClient = &http.Client{Transport: transport, Timeout: 30 * time.Second}
	config.SetPartnerClientId(clientID)
	config.SetConsumerKey(consumerKey)

	return &SnapTradeClient{
		client: snaptrade.NewAPIClient(config),
	}, nil
}

func (c *SnapTradeClient) CreateUser(userID string) (contract.UserRegistration, contract.ResponseMeta, error) {
	body := snaptrade.NewSnapTradeRegisterUserRequestBody(userID)
	result, response, err := c.client.AuthenticationApi.RegisterSnapTradeUser(*body).Execute()
	if err != nil {
		return contract.UserRegistration{}, ResponseMeta(response), responseError(response, err)
	}
	return contract.UserRegistration{
		UserID:     result.GetUserId(),
		UserSecret: result.GetUserSecret(),
	}, ResponseMeta(response), nil
}

func (c *SnapTradeClient) DeleteUser(userID string) (contract.ResponseMeta, error) {
	_, response, err := c.client.AuthenticationApi.DeleteSnapTradeUser(userID).Execute()
	if err != nil {
		return ResponseMeta(response), responseError(response, err)
	}
	return ResponseMeta(response), nil
}

func (c *SnapTradeClient) GenerateConnectionPortalURL(
	userID, userSecret, brokerageID, connectionType, customRedirect, reconnect string,
) (contract.ConnectionPortal, contract.ResponseMeta, error) {
	request := c.client.AuthenticationApi.LoginSnapTradeUser(userID, userSecret)
	body := snaptrade.SnapTradeLoginUserRequestBody{}
	if reconnect != "" {
		body.SetReconnect(reconnect)
	} else if brokerageID != "" {
		body.SetBroker(brokerageID)
	}
	if connectionType != "" {
		body.SetConnectionType(connectionType)
	}
	if customRedirect != "" {
		body.SetCustomRedirect(customRedirect)
	}
	request = *request.SnapTradeLoginUserRequestBody(body)

	result, response, err := request.Execute()
	if err != nil {
		return contract.ConnectionPortal{}, ResponseMeta(response), responseError(response, err)
	}
	if result.LoginRedirectURI == nil {
		return contract.ConnectionPortal{}, ResponseMeta(response), fmt.Errorf("SnapTrade returned no connection portal")
	}
	redirect, ok := result.LoginRedirectURI.GetRedirectURIOk()
	if !ok || redirect == nil || *redirect == "" {
		return contract.ConnectionPortal{}, ResponseMeta(response), fmt.Errorf("SnapTrade returned an empty connection portal URL")
	}
	sessionID, _ := result.LoginRedirectURI.GetSessionIdOk()
	portal := contract.ConnectionPortal{RedirectURL: *redirect}
	if sessionID != nil {
		portal.SessionID = *sessionID
	}
	return portal, ResponseMeta(response), nil
}

func (c *SnapTradeClient) GetConnection(
	userID, userSecret, connectionID string,
) (contract.Connection, contract.ResponseMeta, error) {
	result, response, err := c.client.ConnectionsApi.
		DetailBrokerageAuthorization(connectionID, userID, userSecret).
		Execute()
	if err != nil {
		return contract.Connection{}, ResponseMeta(response), responseError(response, err)
	}
	connection, err := normalizeConnection(result)
	return connection, ResponseMeta(response), err
}

func (c *SnapTradeClient) ListConnections(
	userID, userSecret string,
) ([]contract.Connection, contract.ResponseMeta, error) {
	result, response, err := c.client.ConnectionsApi.ListBrokerageAuthorizations(userID, userSecret).Execute()
	if err != nil {
		return nil, ResponseMeta(response), responseError(response, err)
	}
	connections := make([]contract.Connection, 0, len(result))
	for index := range result {
		connection, normalizeErr := normalizeConnection(&result[index])
		if normalizeErr != nil {
			return nil, ResponseMeta(response), normalizeErr
		}
		connections = append(connections, connection)
	}
	return connections, ResponseMeta(response), nil
}

func (c *SnapTradeClient) RefreshConnection(
	userID, userSecret, connectionID string,
) (contract.RefreshResult, contract.ResponseMeta, error) {
	_, response, err := c.client.ConnectionsApi.
		RefreshBrokerageAuthorization(connectionID, userID, userSecret).
		Execute()
	if err != nil {
		return contract.RefreshResult{}, ResponseMeta(response), responseError(response, err)
	}
	return contract.RefreshResult{ConnectionID: connectionID, Status: "queued"}, ResponseMeta(response), nil
}

func (c *SnapTradeClient) DeleteConnection(
	userID, userSecret, connectionID string,
) (contract.ResponseMeta, error) {
	response, err := c.client.ConnectionsApi.
		RemoveBrokerageAuthorization(connectionID, userID, userSecret).
		Execute()
	if err != nil {
		return ResponseMeta(response), responseError(response, err)
	}
	return ResponseMeta(response), nil
}

func (c *SnapTradeClient) ListAccounts(
	userID, userSecret string,
) ([]contract.Account, contract.ResponseMeta, error) {
	result, response, err := c.client.AccountInformationApi.ListUserAccounts(userID, userSecret).Execute()
	if err != nil {
		return nil, ResponseMeta(response), responseError(response, err)
	}
	accounts := make([]contract.Account, 0, len(result))
	for index := range result {
		account, normalizeErr := normalizeAccount(&result[index])
		if normalizeErr != nil {
			return nil, ResponseMeta(response), normalizeErr
		}
		accounts = append(accounts, account)
	}
	return accounts, ResponseMeta(response), nil
}

func (c *SnapTradeClient) GetAccount(
	userID, userSecret, accountID string,
) (contract.Account, contract.ResponseMeta, error) {
	result, response, err := c.client.AccountInformationApi.
		GetUserAccountDetails(userID, userSecret, accountID).
		Execute()
	if err != nil {
		return contract.Account{}, ResponseMeta(response), responseError(response, err)
	}
	account, normalizeErr := normalizeAccount(result)
	return account, ResponseMeta(response), normalizeErr
}

func (c *SnapTradeClient) GetPortfolioSnapshot(
	userID, userSecret, accountID string,
) (contract.PortfolioSnapshot, contract.ResponseMeta, error) {
	account, meta, err := c.GetAccount(userID, userSecret, accountID)
	if err != nil {
		return contract.PortfolioSnapshot{}, meta, err
	}

	snapshot := contract.PortfolioSnapshot{
		AccountID:  accountID,
		Positions:  []contract.Position{},
		Balances:   []contract.Balance{},
		Orders:     []contract.Order{},
		TotalValue: account.TotalValue,
	}
	if account.SyncStatus != nil && account.SyncStatus.Holdings != nil {
		snapshot.HoldingsUnavailable = account.SyncStatus.Holdings.HoldingsUnavailable
	}
	if snapshot.HoldingsUnavailable {
		return snapshot, meta, nil
	}

	positions, response, err := c.client.AccountInformationApi.
		GetAllAccountPositions(userID, userSecret, accountID).
		Execute()
	meta = MergeMeta(meta, ResponseMeta(response))
	if err != nil {
		return contract.PortfolioSnapshot{}, meta, responseError(response, err)
	}
	normalizedPositions, asOf, err := normalizePositions(positions)
	if err != nil {
		return contract.PortfolioSnapshot{}, meta, err
	}
	snapshot.Positions = normalizedPositions
	snapshot.AsOf = asOf

	balances, response, err := c.client.AccountInformationApi.
		GetUserAccountBalance(userID, userSecret, accountID).
		Execute()
	meta = MergeMeta(meta, ResponseMeta(response))
	if err != nil {
		return contract.PortfolioSnapshot{}, meta, responseError(response, err)
	}
	snapshot.Balances, err = normalizeBalances(balances)
	if err != nil {
		return contract.PortfolioSnapshot{}, meta, err
	}

	orders, response, err := c.client.AccountInformationApi.
		GetUserAccountOrders(userID, userSecret, accountID).
		Execute()
	meta = MergeMeta(meta, ResponseMeta(response))
	if err != nil {
		return contract.PortfolioSnapshot{}, meta, responseError(response, err)
	}
	snapshot.Orders, err = normalizeOrders(orders)
	if err != nil {
		return contract.PortfolioSnapshot{}, meta, err
	}
	snapshot.Complete = true
	return snapshot, meta, nil
}

func (c *SnapTradeClient) GetActivities(
	userID, userSecret, accountID string,
	startDate, endDate, activityType *string,
	offset, limit *int32,
) (contract.ActivitiesPage, contract.ResponseMeta, error) {
	request := c.client.AccountInformationApi.GetAccountActivities(accountID, userID, userSecret)
	if startDate != nil {
		request = *request.StartDate(*startDate)
	}
	if endDate != nil {
		request = *request.EndDate(*endDate)
	}
	if activityType != nil {
		request = *request.Type_(*activityType)
	}
	if offset != nil {
		request = *request.Offset(*offset)
	}
	if limit != nil {
		request = *request.Limit(*limit)
	}

	result, response, err := request.Execute()
	if err != nil {
		return contract.ActivitiesPage{}, ResponseMeta(response), responseError(response, err)
	}
	var raw struct {
		Data       []contract.Activity  `json:"data"`
		Pagination *contract.Pagination `json:"pagination"`
	}
	if err := decodeViaJSON(result, &raw); err != nil {
		return contract.ActivitiesPage{}, ResponseMeta(response), fmt.Errorf("normalize activities: %w", err)
	}
	return contract.ActivitiesPage{Activities: raw.Data, Pagination: raw.Pagination}, ResponseMeta(response), nil
}

func normalizeConnection(value *snaptrade.BrokerageAuthorization) (contract.Connection, error) {
	var raw struct {
		ID                string  `json:"id"`
		Name              *string `json:"name"`
		Type              *string `json:"type"`
		Disabled          *bool   `json:"disabled"`
		DisabledDate      *string `json:"disabled_date"`
		DataFreshnessMode string  `json:"data_freshness_mode"`
	}
	if err := decodeViaJSON(value, &raw); err != nil {
		return contract.Connection{}, fmt.Errorf("normalize connection: %w", err)
	}
	if raw.DataFreshnessMode != "realtime" && raw.DataFreshnessMode != "delayed" {
		raw.DataFreshnessMode = "unknown"
	}
	return contract.Connection{
		ID:                raw.ID,
		Name:              raw.Name,
		Type:              raw.Type,
		Disabled:          raw.Disabled != nil && *raw.Disabled,
		DisabledDate:      raw.DisabledDate,
		DataFreshnessMode: raw.DataFreshnessMode,
	}, nil
}

func normalizeAccount(value *snaptrade.Account) (contract.Account, error) {
	var raw struct {
		ID                     string               `json:"id"`
		Name                   *string              `json:"name"`
		Number                 *string              `json:"number"`
		InstitutionName        *string              `json:"institution_name"`
		BrokerageAuthorization *string              `json:"brokerage_authorization"`
		SyncStatus             *contract.SyncStatus `json:"sync_status"`
		Balance                *struct {
			Total *contract.Money `json:"total"`
		} `json:"balance"`
	}
	if err := decodeViaJSON(value, &raw); err != nil {
		return contract.Account{}, fmt.Errorf("normalize account: %w", err)
	}
	account := contract.Account{
		ID:                     raw.ID,
		Name:                   raw.Name,
		Number:                 raw.Number,
		InstitutionName:        raw.InstitutionName,
		BrokerageAuthorization: raw.BrokerageAuthorization,
		SyncStatus:             raw.SyncStatus,
	}
	if raw.Balance != nil {
		account.TotalValue = raw.Balance.Total
	}
	return account, nil
}

func normalizePositions(value *snaptrade.AllAccountPositionsResponse) ([]contract.Position, *string, error) {
	var raw struct {
		Results []struct {
			Instrument struct {
				Kind           string  `json:"kind"`
				ID             string  `json:"id"`
				Symbol         string  `json:"symbol"`
				RawSymbol      *string `json:"raw_symbol"`
				Description    *string `json:"description"`
				Currency       *string `json:"currency"`
				OptionType     string  `json:"option_type"`
				StrikePrice    float64 `json:"strike_price"`
				ExpirationDate string  `json:"expiration_date"`
				Multiplier     float64 `json:"multiplier"`
				Underlying     struct {
					Symbol string `json:"symbol"`
				} `json:"underlying"`
			} `json:"instrument"`
			Units     *float64 `json:"units"`
			Price     *float64 `json:"price"`
			CostBasis *float64 `json:"cost_basis"`
			Currency  *string  `json:"currency"`
		} `json:"results"`
		DataFreshness struct {
			AsOf *string `json:"as_of"`
		} `json:"data_freshness"`
	}
	if err := decodeViaJSON(value, &raw); err != nil {
		return nil, nil, fmt.Errorf("normalize positions: %w", err)
	}
	positions := make([]contract.Position, 0, len(raw.Results))
	for _, item := range raw.Results {
		currency := item.Currency
		if currency == nil {
			currency = item.Instrument.Currency
		}
		position := contract.Position{
			InstrumentID:         item.Instrument.ID,
			Kind:                 item.Instrument.Kind,
			Symbol:               item.Instrument.Symbol,
			RawSymbol:            item.Instrument.RawSymbol,
			Description:          item.Instrument.Description,
			Currency:             currency,
			Units:                item.Units,
			Price:                item.Price,
			AveragePurchasePrice: item.CostBasis,
		}
		if item.Instrument.Kind == "option" {
			position.Option = &contract.OptionDetails{
				OptionType:       item.Instrument.OptionType,
				StrikePrice:      item.Instrument.StrikePrice,
				ExpirationDate:   item.Instrument.ExpirationDate,
				Multiplier:       item.Instrument.Multiplier,
				UnderlyingSymbol: item.Instrument.Underlying.Symbol,
			}
		}
		positions = append(positions, position)
	}
	return positions, raw.DataFreshness.AsOf, nil
}

func normalizeBalances(values []snaptrade.Balance) ([]contract.Balance, error) {
	var raw []struct {
		Currency struct {
			Code string `json:"code"`
		} `json:"currency"`
		Cash        *float64 `json:"cash"`
		BuyingPower *float64 `json:"buying_power"`
	}
	if err := decodeViaJSON(values, &raw); err != nil {
		return nil, fmt.Errorf("normalize balances: %w", err)
	}
	balances := make([]contract.Balance, 0, len(raw))
	for _, item := range raw {
		balances = append(balances, contract.Balance{
			Currency: item.Currency.Code, Cash: item.Cash, BuyingPower: item.BuyingPower,
		})
	}
	return balances, nil
}

func normalizeOrders(values []snaptrade.AccountOrderRecord) ([]contract.Order, error) {
	var raw []struct {
		BrokerageOrderID string `json:"brokerage_order_id"`
		UniversalSymbol  *struct {
			Symbol string `json:"symbol"`
		} `json:"universal_symbol"`
		OptionSymbol *struct {
			Ticker string `json:"ticker"`
		} `json:"option_symbol"`
		Status     *string  `json:"status"`
		Action     *string  `json:"action"`
		OrderType  *string  `json:"order_type"`
		Units      *float64 `json:"units"`
		Price      *float64 `json:"price"`
		TimePlaced *string  `json:"time_placed"`
	}
	if err := decodeViaJSON(values, &raw); err != nil {
		return nil, fmt.Errorf("normalize orders: %w", err)
	}
	orders := make([]contract.Order, 0, len(raw))
	for _, item := range raw {
		var symbol, optionSymbol *string
		if item.UniversalSymbol != nil {
			symbol = &item.UniversalSymbol.Symbol
		}
		if item.OptionSymbol != nil {
			optionSymbol = &item.OptionSymbol.Ticker
		}
		orders = append(orders, contract.Order{
			BrokerageOrderID: item.BrokerageOrderID,
			Symbol:           symbol, OptionSymbol: optionSymbol, Status: item.Status,
			Action: item.Action, OrderType: item.OrderType, Units: item.Units,
			Price: item.Price, TimePlaced: item.TimePlaced,
		})
	}
	return orders, nil
}

func decodeViaJSON(input, output any) error {
	encoded, err := json.Marshal(input)
	if err != nil {
		return err
	}
	return json.Unmarshal(encoded, output)
}

func responseError(response *http.Response, wrapped error) error {
	if response == nil {
		return wrapped
	}
	var body []byte
	var sdkError interface{ Body() []byte }
	if errors.As(wrapped, &sdkError) {
		body = sdkError.Body()
	}
	if len(body) == 0 && response.Body != nil {
		body, _ = io.ReadAll(io.LimitReader(response.Body, 64*1024))
	}
	if len(body) > 64*1024 {
		body = body[:64*1024]
	}
	return NewSnapTradeAPIError(response, body, wrapped)
}

func MergeMeta(left, right contract.ResponseMeta) contract.ResponseMeta {
	if right.RequestID != "" {
		left.RequestID = right.RequestID
	}
	if right.RateLimit == nil {
		return left
	}
	if left.RateLimit == nil {
		left.RateLimit = right.RateLimit
		return left
	}
	mergeMinimum := func(current **int, incoming *int) {
		if incoming != nil && (*current == nil || *incoming < **current) {
			value := *incoming
			*current = &value
		}
	}
	mergeMinimum(&left.RateLimit.Remaining, right.RateLimit.Remaining)
	mergeMinimum(&left.RateLimit.AccountRemaining, right.RateLimit.AccountRemaining)
	mergeMinimum(&left.RateLimit.ResetSeconds, right.RateLimit.ResetSeconds)
	mergeMinimum(&left.RateLimit.AccountReset, right.RateLimit.AccountReset)
	if left.RateLimit.Limit == nil {
		left.RateLimit.Limit = right.RateLimit.Limit
	}
	if left.RateLimit.AccountLimit == nil {
		left.RateLimit.AccountLimit = right.RateLimit.AccountLimit
	}
	return left
}
