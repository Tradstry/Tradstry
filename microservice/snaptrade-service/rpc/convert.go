package rpc

import (
	"snaptrade-service/contract"
	pb "snaptrade-service/gen/snaptrade/v1"
)

func responseMeta(value contract.ResponseMeta) *pb.ResponseMeta {
	result := &pb.ResponseMeta{ContractVersion: contract.Version}
	if value.RequestID != "" {
		result.RequestId = &value.RequestID
	}
	if value.RateLimit != nil {
		result.RateLimit = &pb.RateLimit{
			Limit:               intPointer(value.RateLimit.Limit),
			Remaining:           intPointer(value.RateLimit.Remaining),
			ResetSeconds:        intPointer(value.RateLimit.ResetSeconds),
			AccountLimit:        intPointer(value.RateLimit.AccountLimit),
			AccountRemaining:    intPointer(value.RateLimit.AccountRemaining),
			AccountResetSeconds: intPointer(value.RateLimit.AccountReset),
		}
	}
	return result
}

func intPointer(value *int) *int64 {
	if value == nil {
		return nil
	}
	converted := int64(*value)
	return &converted
}

func connection(value contract.Connection) *pb.Connection {
	return &pb.Connection{
		Id: value.ID, Name: value.Name, ConnectionType: value.Type,
		Disabled: value.Disabled, DisabledDate: value.DisabledDate,
		DataFreshnessMode: value.DataFreshnessMode,
	}
}

func money(value *contract.Money) *pb.Money {
	if value == nil {
		return nil
	}
	return &pb.Money{Amount: value.Amount, Currency: value.Currency}
}

func account(value contract.Account) *pb.Account {
	return &pb.Account{
		Id: value.ID, Name: value.Name, Number: value.Number,
		InstitutionName:        value.InstitutionName,
		BrokerageAuthorization: value.BrokerageAuthorization,
		TotalValue:             money(value.TotalValue), SyncStatus: syncStatus(value.SyncStatus),
	}
}

func syncStatus(value *contract.SyncStatus) *pb.SyncStatus {
	if value == nil {
		return nil
	}
	result := &pb.SyncStatus{}
	if value.Transactions != nil {
		result.Transactions = &pb.TransactionsSyncStatus{
			InitialSyncCompleted: value.Transactions.InitialSyncCompleted,
			LastSuccessfulSync:   value.Transactions.LastSuccessfulSync,
			FirstTransactionDate: value.Transactions.FirstTransactionDate,
		}
	}
	if value.Holdings != nil {
		result.Holdings = &pb.HoldingsSyncStatus{
			InitialSyncCompleted: value.Holdings.InitialSyncCompleted,
			LastSuccessfulSync:   value.Holdings.LastSuccessfulSync,
			HoldingsUnavailable:  value.Holdings.HoldingsUnavailable,
		}
	}
	return result
}

func portfolio(value contract.PortfolioSnapshot) *pb.PortfolioSnapshot {
	result := &pb.PortfolioSnapshot{
		AccountId: value.AccountID, AsOf: value.AsOf, Complete: value.Complete,
		HoldingsUnavailable: value.HoldingsUnavailable, TotalValue: money(value.TotalValue),
		Positions: make([]*pb.Position, 0, len(value.Positions)),
		Balances:  make([]*pb.Balance, 0, len(value.Balances)),
		Orders:    make([]*pb.Order, 0, len(value.Orders)),
	}
	for _, item := range value.Positions {
		position := &pb.Position{
			InstrumentId: item.InstrumentID, Kind: item.Kind, Symbol: item.Symbol,
			RawSymbol: item.RawSymbol, Description: item.Description, Currency: item.Currency,
			Units: item.Units, Price: item.Price, AveragePurchasePrice: item.AveragePurchasePrice,
		}
		if item.Option != nil {
			position.Option = &pb.OptionDetails{
				OptionType: item.Option.OptionType, StrikePrice: item.Option.StrikePrice,
				ExpirationDate: item.Option.ExpirationDate, Multiplier: item.Option.Multiplier,
				UnderlyingSymbol: item.Option.UnderlyingSymbol,
			}
		}
		result.Positions = append(result.Positions, position)
	}
	for _, item := range value.Balances {
		result.Balances = append(result.Balances, &pb.Balance{
			Currency: item.Currency, Cash: item.Cash, BuyingPower: item.BuyingPower,
		})
	}
	for _, item := range value.Orders {
		result.Orders = append(result.Orders, &pb.Order{
			BrokerageOrderId: item.BrokerageOrderID, Symbol: item.Symbol,
			OptionSymbol: item.OptionSymbol, Status: item.Status, Action: item.Action,
			OrderType: item.OrderType, Units: item.Units, Price: item.Price,
			TimePlaced: item.TimePlaced,
		})
	}
	return result
}

func activities(value contract.ActivitiesPage) *pb.ActivitiesPage {
	result := &pb.ActivitiesPage{Activities: make([]*pb.Activity, 0, len(value.Activities))}
	if value.Pagination != nil {
		result.Pagination = &pb.Pagination{
			Offset: value.Pagination.Offset, Limit: value.Pagination.Limit, Total: value.Pagination.Total,
		}
	}
	for _, item := range value.Activities {
		result.Activities = append(result.Activities, &pb.Activity{
			Id: item.ID, Symbol: activitySymbol(item.Symbol), OptionSymbol: optionSymbol(item.OptionSymbol),
			Price: item.Price, Units: item.Units, Amount: item.Amount, Currency: currency(item.Currency),
			ActivityType: item.Type, OptionType: item.OptionType, Description: item.Description,
			TradeDate: item.TradeDate, SettlementDate: item.SettlementDate, Fee: item.Fee,
			FxRate: item.FXRate, Institution: item.Institution,
			ExternalReferenceId: item.ExternalReferenceID,
		})
	}
	return result
}

func activitySymbol(value *contract.ActivitySymbol) *pb.ActivitySymbol {
	if value == nil {
		return nil
	}
	return &pb.ActivitySymbol{
		Id: value.ID, Symbol: value.Symbol, RawSymbol: value.RawSymbol,
		Description: value.Description, Currency: currency(value.Currency),
	}
}

func currency(value *contract.Currency) *pb.Currency {
	if value == nil {
		return nil
	}
	return &pb.Currency{Id: value.ID, Code: value.Code, Name: value.Name}
}

func optionSymbol(value *contract.OptionSymbol) *pb.OptionSymbol {
	if value == nil {
		return nil
	}
	return &pb.OptionSymbol{
		Id: value.ID, Ticker: value.Ticker, OptionType: value.OptionType,
		StrikePrice: value.StrikePrice, ExpirationDate: value.ExpirationDate,
		IsMiniOption: value.IsMiniOption, UnderlyingSymbol: activitySymbol(value.UnderlyingSymbol),
	}
}
