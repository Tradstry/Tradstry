import type { GraphQLFetcher } from "@tradstry/app-ui/lib/client";
import type {
	MarketArticle,
	MarketCandle,
	MarketMonitor,
	MarketQuote,
	MarketReport,
	MarketSearchResult,
	MarketTranscript,
	MarketTranscriptRef,
	MarketWatchlist,
} from "@tradstry/app-ui/lib/types/market";

const QUOTE_FIELDS = `symbol name price change changePercent currency exchange marketState marketTime isStale`;

export const MARKET_PRICE_UPDATES_SUBSCRIPTION = `
  subscription MarketPriceUpdates($symbols: [String!]!) {
    marketPriceUpdates(symbols: $symbols) {
      symbol price change changePercent currency exchange marketState marketTime
    }
  }
`;

export async function fetchQuotes(fetcher: GraphQLFetcher, symbols: string[]) {
	const data = await fetcher<{
		marketQuotes: {
			quotes: MarketQuote[];
			errors: { symbol: string; message: string }[];
			fetchedAt: string;
		};
	}>(
		`query MarketQuotes($symbols: [String!]!) { marketQuotes(symbols: $symbols) { quotes { ${QUOTE_FIELDS} } errors { symbol message } fetchedAt } }`,
		{ symbols },
	);
	return data.marketQuotes;
}

export async function search(fetcher: GraphQLFetcher, query: string) {
	const data = await fetcher<{ marketSearch: MarketSearchResult[] }>(
		`query MarketSearch($query: String!) { marketSearch(query: $query) { symbol name exchange securityType } }`,
		{ query },
	);
	return data.marketSearch;
}

export async function fetchChart(
	fetcher: GraphQLFetcher,
	symbol: string,
	range: string,
) {
	const data = await fetcher<{ marketChart: MarketCandle[] }>(
		`query MarketChart($symbol: String!, $range: String) { marketChart(symbol: $symbol, range: $range) { timestamp open high low close volume } }`,
		{ symbol, range },
	);
	return data.marketChart;
}

export async function fetchNews(fetcher: GraphQLFetcher, symbol: string) {
	const data = await fetcher<{ marketNews: MarketArticle[] }>(
		`query MarketNews($symbol: String!) { marketNews(symbol: $symbol) { title url source publishedAt imageUrl } }`,
		{ symbol },
	);
	return data.marketNews;
}

export async function fetchFinancials(fetcher: GraphQLFetcher, symbol: string) {
	const data = await fetcher<{ marketFinancials: Record<string, unknown> }>(
		`query MarketFinancials($symbol: String!) { marketFinancials(symbol: $symbol) }`,
		{ symbol },
	);
	return data.marketFinancials;
}

export async function fetchCompany(fetcher: GraphQLFetcher, symbol: string) {
	const data = await fetcher<{ marketCompany: Record<string, unknown> }>(
		`query MarketCompany($symbol: String!) { marketCompany(symbol: $symbol) }`,
		{ symbol },
	);
	return data.marketCompany;
}

export async function fetchTranscriptList(
	fetcher: GraphQLFetcher,
	symbol: string,
) {
	const data = await fetcher<{ marketTranscriptList: MarketTranscriptRef[] }>(
		`query MarketTranscriptList($symbol: String!) { marketTranscriptList(symbol: $symbol) { symbol quarter year date } }`,
		{ symbol },
	);
	return data.marketTranscriptList;
}

export async function fetchTranscript(
	fetcher: GraphQLFetcher,
	symbol: string,
	quarter: number,
	year: number,
) {
	const data = await fetcher<{ marketTranscript: MarketTranscript }>(
		`query MarketTranscript($symbol: String!, $quarter: Int!, $year: Int!) { marketTranscript(symbol: $symbol, quarter: $quarter, year: $year) { symbol quarter year date content sourceUrl } }`,
		{ symbol, quarter, year },
	);
	return data.marketTranscript;
}

export async function fetchWatchlists(
	fetcher: GraphQLFetcher,
	workspaceId: string,
) {
	const data = await fetcher<{ marketWatchlists: MarketWatchlist[] }>(
		`query MarketWatchlists($workspaceId: String!) { marketWatchlists(workspaceId: $workspaceId) { id name symbols createdAt } }`,
		{ workspaceId },
	);
	return data.marketWatchlists;
}

export async function createWatchlist(
	fetcher: GraphQLFetcher,
	workspaceId: string,
	name: string,
) {
	const data = await fetcher<{ createMarketWatchlist: MarketWatchlist }>(
		`mutation CreateMarketWatchlist($workspaceId: String!, $name: String!) { createMarketWatchlist(workspaceId: $workspaceId, name: $name) { id name symbols createdAt } }`,
		{ workspaceId, name },
	);
	return data.createMarketWatchlist;
}

export async function addWatchlistSymbol(
	fetcher: GraphQLFetcher,
	watchlistId: string,
	symbol: string,
) {
	const data = await fetcher<{ addMarketWatchlistSymbol: boolean }>(
		`mutation AddMarketWatchlistSymbol($watchlistId: String!, $symbol: String!) { addMarketWatchlistSymbol(watchlistId: $watchlistId, symbol: $symbol) }`,
		{ watchlistId, symbol },
	);
	return data.addMarketWatchlistSymbol;
}

export async function removeWatchlistSymbol(
	fetcher: GraphQLFetcher,
	watchlistId: string,
	symbol: string,
) {
	const data = await fetcher<{ removeMarketWatchlistSymbol: boolean }>(
		`mutation RemoveMarketWatchlistSymbol($watchlistId: String!, $symbol: String!) { removeMarketWatchlistSymbol(watchlistId: $watchlistId, symbol: $symbol) }`,
		{ watchlistId, symbol },
	);
	return data.removeMarketWatchlistSymbol;
}

export async function fetchReports(
	fetcher: GraphQLFetcher,
	workspaceId: string,
) {
	const data = await fetcher<{ marketReports: MarketReport[] }>(
		`query MarketReports($workspaceId: String!) { marketReports(workspaceId: $workspaceId) { id symbol title body sources createdAt } }`,
		{ workspaceId },
	);
	return data.marketReports;
}

export async function generateReport(
	fetcher: GraphQLFetcher,
	workspaceId: string,
	symbol: string,
	focus?: string,
) {
	const data = await fetcher<{ generateMarketReport: MarketReport }>(
		`mutation GenerateMarketReport($workspaceId: String!, $symbol: String!, $focus: String) { generateMarketReport(workspaceId: $workspaceId, symbol: $symbol, focus: $focus) { id symbol title body sources createdAt } }`,
		{ workspaceId, symbol, focus },
	);
	return data.generateMarketReport;
}

export async function fetchMonitors(
	fetcher: GraphQLFetcher,
	workspaceId: string,
) {
	const data = await fetcher<{ marketMonitors: MarketMonitor[] }>(
		`query MarketMonitors($workspaceId: String!) { marketMonitors(workspaceId: $workspaceId) { id symbol name condition threshold enabled lastTriggeredAt createdAt } }`,
		{ workspaceId },
	);
	return data.marketMonitors;
}

export async function createMonitor(
	fetcher: GraphQLFetcher,
	workspaceId: string,
	input: {
		symbol: string;
		name: string;
		condition: "ABOVE" | "BELOW";
		threshold: number;
	},
) {
	const data = await fetcher<{ createMarketMonitor: MarketMonitor }>(
		`mutation CreateMarketMonitor($workspaceId: String!, $input: CreateMarketMonitorInput!) { createMarketMonitor(workspaceId: $workspaceId, input: $input) { id symbol name condition threshold enabled lastTriggeredAt createdAt } }`,
		{ workspaceId, input },
	);
	return data.createMarketMonitor;
}

export async function deleteMonitor(fetcher: GraphQLFetcher, id: string) {
	const data = await fetcher<{ deleteMarketMonitor: boolean }>(
		`mutation DeleteMarketMonitor($id: String!) { deleteMarketMonitor(id: $id) }`,
		{ id },
	);
	return data.deleteMarketMonitor;
}

export async function evaluateMonitors(
	fetcher: GraphQLFetcher,
	workspaceId: string,
) {
	const data = await fetcher<{ evaluateMarketMonitors: number }>(
		`mutation EvaluateMarketMonitors($workspaceId: String!) { evaluateMarketMonitors(workspaceId: $workspaceId) }`,
		{ workspaceId },
	);
	return data.evaluateMarketMonitors;
}
