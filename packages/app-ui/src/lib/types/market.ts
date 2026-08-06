export type MarketQuote = {
	symbol: string;
	name: string;
	price: number | null;
	change: number | null;
	changePercent: number | null;
	currency: string | null;
	exchange: string | null;
	marketState: string;
	marketTime: string | null;
	isStale: boolean;
};

export type MarketPriceUpdate = {
	symbol: string;
	price: number;
	change: number;
	changePercent: number;
	currency: string;
	exchange: string;
	marketState: string;
	marketTime: string;
};

export type MarketSearchResult = {
	symbol: string;
	name: string;
	exchange: string | null;
	securityType: string | null;
};

export type MarketCandle = {
	timestamp: number;
	open: number;
	high: number;
	low: number;
	close: number;
	volume: number;
};

export type MarketArticle = {
	title: string;
	url: string;
	source: string;
	publishedAt: string;
	imageUrl: string | null;
};

export type MarketTranscriptRef = {
	symbol: string;
	quarter: number;
	year: number;
	date: string | null;
};

export type MarketTranscript = MarketTranscriptRef & {
	content: string;
	sourceUrl: string;
};

export type MarketWatchlist = {
	id: string;
	name: string;
	symbols: string[];
	createdAt: string;
};

export type MarketReport = {
	id: string;
	symbol: string;
	title: string;
	body: string;
	sources: string[];
	createdAt: string;
};

export type MarketMonitor = {
	id: string;
	symbol: string;
	name: string;
	condition: "ABOVE" | "BELOW";
	threshold: number;
	enabled: boolean;
	lastTriggeredAt: string | null;
	createdAt: string;
};
