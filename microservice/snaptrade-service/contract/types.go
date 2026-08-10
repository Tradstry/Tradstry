package contract

const Version = "2026-08-09"

type ResponseMeta struct {
	RequestID string     `json:"request_id,omitempty"`
	RateLimit *RateLimit `json:"rate_limit,omitempty"`
}

type RateLimit struct {
	Limit            *int `json:"limit,omitempty"`
	Remaining        *int `json:"remaining,omitempty"`
	ResetSeconds     *int `json:"reset_seconds,omitempty"`
	AccountLimit     *int `json:"account_limit,omitempty"`
	AccountRemaining *int `json:"account_remaining,omitempty"`
	AccountReset     *int `json:"account_reset_seconds,omitempty"`
}

type UserRegistration struct {
	UserID     string `json:"user_id"`
	UserSecret string `json:"user_secret"`
}

type ConnectionPortal struct {
	RedirectURL string `json:"redirect_url"`
	SessionID   string `json:"session_id"`
}

type Connection struct {
	ID                string  `json:"id"`
	Name              *string `json:"name,omitempty"`
	Type              *string `json:"type,omitempty"`
	Disabled          bool    `json:"disabled"`
	DisabledDate      *string `json:"disabled_date,omitempty"`
	DataFreshnessMode string  `json:"data_freshness_mode"`
}

type RefreshResult struct {
	ConnectionID string `json:"connection_id"`
	Status       string `json:"status"`
}

type Account struct {
	ID                     string      `json:"id"`
	Name                   *string     `json:"name,omitempty"`
	Number                 *string     `json:"number,omitempty"`
	InstitutionName        *string     `json:"institution_name,omitempty"`
	BrokerageAuthorization *string     `json:"brokerage_authorization,omitempty"`
	TotalValue             *Money      `json:"total_value,omitempty"`
	SyncStatus             *SyncStatus `json:"sync_status,omitempty"`
}

type SyncStatus struct {
	Transactions *TransactionsSyncStatus `json:"transactions,omitempty"`
	Holdings     *HoldingsSyncStatus     `json:"holdings,omitempty"`
}

type TransactionsSyncStatus struct {
	InitialSyncCompleted *bool   `json:"initial_sync_completed,omitempty"`
	LastSuccessfulSync   *string `json:"last_successful_sync,omitempty"`
	FirstTransactionDate *string `json:"first_transaction_date,omitempty"`
}

type HoldingsSyncStatus struct {
	InitialSyncCompleted *bool   `json:"initial_sync_completed,omitempty"`
	LastSuccessfulSync   *string `json:"last_successful_sync,omitempty"`
	HoldingsUnavailable  bool    `json:"holdings_unavailable"`
}

type Money struct {
	Amount   *float64 `json:"amount,omitempty"`
	Currency *string  `json:"currency,omitempty"`
}

type PortfolioSnapshot struct {
	AccountID           string     `json:"account_id"`
	AsOf                *string    `json:"as_of,omitempty"`
	Complete            bool       `json:"complete"`
	HoldingsUnavailable bool       `json:"holdings_unavailable"`
	Positions           []Position `json:"positions"`
	Balances            []Balance  `json:"balances"`
	Orders              []Order    `json:"orders"`
	TotalValue          *Money     `json:"total_value,omitempty"`
}

type Position struct {
	InstrumentID         string         `json:"instrument_id"`
	Kind                 string         `json:"kind"`
	Symbol               string         `json:"symbol"`
	RawSymbol            *string        `json:"raw_symbol,omitempty"`
	Description          *string        `json:"description,omitempty"`
	Currency             *string        `json:"currency,omitempty"`
	Units                *float64       `json:"units,omitempty"`
	Price                *float64       `json:"price,omitempty"`
	AveragePurchasePrice *float64       `json:"average_purchase_price,omitempty"`
	Option               *OptionDetails `json:"option,omitempty"`
}

type OptionDetails struct {
	OptionType       string  `json:"option_type"`
	StrikePrice      float64 `json:"strike_price"`
	ExpirationDate   string  `json:"expiration_date"`
	Multiplier       float64 `json:"multiplier"`
	UnderlyingSymbol string  `json:"underlying_symbol,omitempty"`
}

type Balance struct {
	Currency    string   `json:"currency"`
	Cash        *float64 `json:"cash,omitempty"`
	BuyingPower *float64 `json:"buying_power,omitempty"`
}

type Order struct {
	BrokerageOrderID string   `json:"brokerage_order_id"`
	Symbol           *string  `json:"symbol,omitempty"`
	OptionSymbol     *string  `json:"option_symbol,omitempty"`
	Status           *string  `json:"status,omitempty"`
	Action           *string  `json:"action,omitempty"`
	OrderType        *string  `json:"order_type,omitempty"`
	Units            *float64 `json:"units,omitempty"`
	Price            *float64 `json:"price,omitempty"`
	TimePlaced       *string  `json:"time_placed,omitempty"`
}

type Pagination struct {
	Offset *int32 `json:"offset,omitempty"`
	Limit  *int32 `json:"limit,omitempty"`
	Total  *int32 `json:"total,omitempty"`
}

type ActivitiesPage struct {
	Activities []Activity  `json:"activities"`
	Pagination *Pagination `json:"pagination,omitempty"`
}

type Activity struct {
	ID                  *string         `json:"id,omitempty"`
	Symbol              *ActivitySymbol `json:"symbol,omitempty"`
	OptionSymbol        *OptionSymbol   `json:"option_symbol,omitempty"`
	Price               *float64        `json:"price,omitempty"`
	Units               *float64        `json:"units,omitempty"`
	Amount              *float64        `json:"amount,omitempty"`
	Currency            *Currency       `json:"currency,omitempty"`
	Type                *string         `json:"type,omitempty"`
	OptionType          *string         `json:"option_type,omitempty"`
	Description         *string         `json:"description,omitempty"`
	TradeDate           *string         `json:"trade_date,omitempty"`
	SettlementDate      *string         `json:"settlement_date,omitempty"`
	Fee                 *float64        `json:"fee,omitempty"`
	FXRate              *float64        `json:"fx_rate,omitempty"`
	Institution         *string         `json:"institution,omitempty"`
	ExternalReferenceID *string         `json:"external_reference_id,omitempty"`
}

type ActivitySymbol struct {
	ID          *string   `json:"id,omitempty"`
	Symbol      *string   `json:"symbol,omitempty"`
	RawSymbol   *string   `json:"raw_symbol,omitempty"`
	Description *string   `json:"description,omitempty"`
	Currency    *Currency `json:"currency,omitempty"`
}

type Currency struct {
	ID   *string `json:"id,omitempty"`
	Code *string `json:"code,omitempty"`
	Name *string `json:"name,omitempty"`
}

type OptionSymbol struct {
	ID               *string         `json:"id,omitempty"`
	Ticker           *string         `json:"ticker,omitempty"`
	OptionType       *string         `json:"option_type,omitempty"`
	StrikePrice      *float64        `json:"strike_price,omitempty"`
	ExpirationDate   *string         `json:"expiration_date,omitempty"`
	IsMiniOption     *bool           `json:"is_mini_option,omitempty"`
	UnderlyingSymbol *ActivitySymbol `json:"underlying_symbol,omitempty"`
}
