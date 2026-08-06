"use client";

import {
  Add01Icon,
  ArrowUpRight01Icon,
  Cancel01Icon,
  Delete02Icon,
  Notification01Icon,
  Search01Icon,
  SparklesIcon,
} from "@hugeicons/core-free-icons";
import { HugeiconsIcon } from "@hugeicons/react";
import { Button } from "@tradstry/app-ui/components/ui/button";
import {
  Card,
  CardContent,
  CardHeader,
  CardTitle,
} from "@tradstry/app-ui/components/ui/card";
import { Input } from "@tradstry/app-ui/components/ui/input";
import { ScrollArea } from "@tradstry/app-ui/components/ui/scroll-area";
import { Skeleton } from "@tradstry/app-ui/components/ui/skeleton";
import {
  Tabs,
  TabsContent,
  TabsList,
  TabsTrigger,
} from "@tradstry/app-ui/components/ui/tabs";
import { useActiveWorkspace } from "@tradstry/app-ui/components/workspaces";
import { useChatStore } from "@tradstry/app-ui/hooks/chat";
import {
  useAddMarketWatchlistSymbol,
  useCreateMarketMonitor,
  useCreateMarketWatchlist,
  useDeleteMarketMonitor,
  useEvaluateMarketMonitors,
  useGenerateMarketReport,
  useMarketChart,
  useMarketCompany,
  useMarketFinancials,
  useMarketMonitors,
  useMarketNews,
  useMarketQuotes,
  useMarketReports,
  useMarketSearch,
  useMarketTranscript,
  useMarketTranscriptList,
  useMarketWatchlists,
  useRemoveMarketWatchlistSymbol,
} from "@tradstry/app-ui/hooks/market";
import type {
  MarketQuote,
  MarketReport,
  MarketTranscriptRef,
} from "@tradstry/app-ui/lib/types/market";
import { cn } from "@tradstry/app-ui/lib/utils";
import { useTradstryPlatform } from "@tradstry/app-ui/platform";
import * as React from "react";
import ReactMarkdown from "react-markdown";
import {
  Area,
  AreaChart,
  CartesianGrid,
  Tooltip as ChartTooltip,
  ResponsiveContainer,
  XAxis,
  YAxis,
} from "recharts";

const DEFAULT_SYMBOLS = ["AAPL", "MSFT", "NVDA", "SPY"];
const CHART_RANGES = ["1D", "5D", "1M", "3M", "6M", "1Y", "5Y"];
const RANGE_SECONDS: Record<string, number> = {
  "1D": 86_400,
  "5D": 5 * 86_400,
  "1M": 31 * 86_400,
  "3M": 93 * 86_400,
  "6M": 186 * 86_400,
  "1Y": 366 * 86_400,
  "5Y": 5 * 366 * 86_400,
};

function money(value: number | null | undefined) {
  return value == null
    ? "—"
    : new Intl.NumberFormat("en-US", {
        style: "currency",
        currency: "USD",
        maximumFractionDigits: 2,
      }).format(value);
}

function compact(value: number) {
  return new Intl.NumberFormat("en-US", {
    notation: "compact",
    maximumFractionDigits: 1,
  }).format(value);
}

function signedPercent(value: number | null | undefined) {
  if (value == null) return "—";
  return `${value >= 0 ? "+" : ""}${value.toFixed(2)}%`;
}

function shortDate(value: string | null | undefined) {
  if (!value) return "Date unavailable";
  const date = new Date(value);
  return Number.isNaN(date.getTime())
    ? value
    : date.toLocaleDateString(undefined, {
        month: "short",
        day: "numeric",
        year: "numeric",
      });
}

function humanize(value: string) {
  const words = value
    .replace(/_/g, " ")
    .replace(/([a-z0-9])([A-Z])/g, "$1 $2")
    .trim()
    .toLowerCase();
  return words ? words[0].toUpperCase() + words.slice(1) : value;
}

function ErrorState({ message }: { message: string }) {
  return (
    <div className="rounded-lg border border-destructive/30 bg-destructive/5 p-4 text-sm text-destructive">
      {message}
    </div>
  );
}

function ResearchChart({ symbol }: { symbol: string }) {
  const [range, setRange] = React.useState("3M");
  const gradientId = React.useId();
  const chart = useMarketChart(symbol, range);
  const sortedData = [...(chart.data ?? [])].sort(
    (a, b) => a.timestamp - b.timestamp,
  );
  const latestTimestamp = sortedData.at(-1)?.timestamp;
  const visibleData = latestTimestamp
    ? sortedData.filter(
        (point) =>
          point.timestamp >= latestTimestamp - (RANGE_SECONDS[range] ?? 0),
      )
    : sortedData;
  const data = visibleData.map((point) => ({
    ...point,
    date: new Date(point.timestamp * 1000).toLocaleDateString(undefined, {
      month: "short",
      day: "numeric",
    }),
  }));
  const firstClose = data[0]?.close;
  const lastClose = data.at(-1)?.close;
  const rangeChange =
    firstClose && lastClose
      ? ((lastClose - firstClose) / firstClose) * 100
      : null;
  const isPositive = (rangeChange ?? 0) >= 0;
  const chartColor = isPositive ? "#059669" : "#e11d48";
  return (
    <Card className="h-[420px] min-w-0">
      <CardHeader className="flex flex-row items-start justify-between gap-4">
        <div>
          <CardTitle>Price</CardTitle>
          <p
            className={cn(
              "mt-1 text-xs font-medium tabular-nums",
              isPositive ? "text-emerald-600" : "text-rose-600",
            )}
          >
            {rangeChange == null
              ? "Waiting for price history"
              : `${signedPercent(rangeChange)} over ${range}`}
          </p>
        </div>
        <div className="flex max-w-full overflow-x-auto rounded-lg bg-muted/70 p-0.5">
          {CHART_RANGES.map((item) => (
            <button
              key={item}
              type="button"
              onClick={() => setRange(item)}
              className={cn(
                "rounded-md px-2 py-1 text-[11px] font-medium text-muted-foreground transition-colors hover:text-foreground",
                range === item &&
                  "bg-background text-foreground shadow-sm ring-1 ring-foreground/5",
              )}
            >
              {item}
            </button>
          ))}
        </div>
      </CardHeader>
      <CardContent className="min-h-0 flex-1 pl-1 pr-3">
        {chart.isLoading ? (
          <Skeleton className="h-full w-full" />
        ) : chart.error ? (
          <ErrorState message={chart.error.message} />
        ) : (
          <ResponsiveContainer width="100%" height="100%">
            <AreaChart
              data={data}
              margin={{ top: 12, right: 12, left: 0, bottom: 0 }}
            >
              <defs>
                <linearGradient id={gradientId} x1="0" y1="0" x2="0" y2="1">
                  <stop offset="0%" stopColor={chartColor} stopOpacity={0.2} />
                  <stop offset="100%" stopColor={chartColor} stopOpacity={0} />
                </linearGradient>
              </defs>
              <CartesianGrid
                strokeDasharray="3 3"
                vertical={false}
                stroke="currentColor"
                opacity={0.08}
              />
              <XAxis
                dataKey="date"
                axisLine={false}
                tickLine={false}
                minTickGap={32}
                fontSize={11}
              />
              <YAxis
                domain={["auto", "auto"]}
                axisLine={false}
                tickLine={false}
                width={58}
                tickFormatter={(value) => `$${Number(value).toFixed(0)}`}
                fontSize={11}
              />
              <ChartTooltip
                formatter={(value) => money(Number(value))}
                contentStyle={{
                  borderRadius: 10,
                  border:
                    "1px solid color-mix(in oklab, currentColor 12%, transparent)",
                  fontSize: 12,
                }}
              />
              <Area
                type="monotone"
                dataKey="close"
                stroke={chartColor}
                strokeWidth={2}
                fill={`url(#${gradientId})`}
              />
            </AreaChart>
          </ResponsiveContainer>
        )}
      </CardContent>
    </Card>
  );
}

function Financials({ symbol }: { symbol: string }) {
  const query = useMarketFinancials(symbol);
  if (query.isLoading) return <Skeleton className="h-80 w-full" />;
  if (query.error) return <ErrorState message={query.error.message} />;
  const statements = Object.entries(query.data ?? {}).filter(
    ([, value]) => value && typeof value === "object",
  );
  return (
    <div className="grid gap-4 xl:grid-cols-3">
      {statements.map(([name, raw]) => {
        const statement =
          (raw as { statement?: Record<string, Record<string, number>> })
            .statement ?? {};
        const metrics = Object.entries(statement)
          .filter(
            ([metric, values]) =>
              Object.keys(values).length && !metric.endsWith("_continuing"),
          )
          .slice(0, 12);
        return (
          <Card key={name}>
            <CardHeader>
              <CardTitle className="capitalize">{humanize(name)}</CardTitle>
            </CardHeader>
            <CardContent className="space-y-2">
              {metrics.length ? (
                metrics.map(([metric, values]) => {
                  const latest = Object.entries(values).sort(([a], [b]) =>
                    b.localeCompare(a),
                  )[0];
                  return (
                    <div
                      key={metric}
                      className="flex items-center justify-between gap-3 border-b border-border/50 py-1.5 last:border-0"
                    >
                      <span className="truncate text-muted-foreground">
                        {humanize(metric)}
                      </span>
                      <span className="font-medium tabular-nums">
                        {latest ? compact(latest[1]) : "—"}
                      </span>
                    </div>
                  );
                })
              ) : (
                <p className="text-muted-foreground">
                  No statement data returned.
                </p>
              )}
            </CardContent>
          </Card>
        );
      })}
    </div>
  );
}

function companyField(data: Record<string, unknown> | undefined, key: string) {
  const value = data?.[key];
  if (value && typeof value === "object" && "raw" in value) {
    return (value as { raw?: unknown }).raw;
  }
  return value;
}

function CompanyOverview({
  symbol,
  quote,
}: {
  symbol: string;
  quote: MarketQuote | undefined;
}) {
  const company = useMarketCompany(symbol);
  const { openExternal } = useTradstryPlatform();
  if (company.isLoading) return <Skeleton className="h-[420px] w-full" />;
  if (company.error) return <ErrorState message={company.error.message} />;
  const data = company.data;
  const summary = companyField(data, "longBusinessSummary");
  const website = companyField(data, "website");
  const metrics: [string, unknown][] = [
    ["Market cap", companyField(data, "marketCap")],
    ["Trailing P/E", companyField(data, "trailingPE")],
    ["52-week high", companyField(data, "fiftyTwoWeekHigh")],
    ["52-week low", companyField(data, "fiftyTwoWeekLow")],
  ];
  return (
    <Card className="h-[420px]">
      <CardHeader className="flex flex-row items-center justify-between">
        <div>
          <CardTitle>Company</CardTitle>
          <p className="mt-1 text-xs text-muted-foreground">
            {quote?.exchange ?? "U.S. market"} · {quote?.marketState ?? "—"}
          </p>
        </div>
        {typeof website === "string" && website ? (
          <Button
            variant="ghost"
            size="icon-sm"
            aria-label={`Open ${symbol} website`}
            onClick={() => void openExternal(website)}
          >
            <HugeiconsIcon icon={ArrowUpRight01Icon} className="size-4" />
          </Button>
        ) : null}
      </CardHeader>
      <CardContent className="flex min-h-0 flex-1 flex-col gap-4">
        <div className="flex flex-wrap gap-1.5 text-[11px] text-muted-foreground">
          {[companyField(data, "sector"), companyField(data, "industry")]
            .filter(Boolean)
            .map((item) => (
              <span
                key={String(item)}
                className="rounded-full bg-muted px-2 py-1"
              >
                {String(item)}
              </span>
            ))}
        </div>
        <div className="grid grid-cols-2 gap-px overflow-hidden rounded-lg bg-border/70 ring-1 ring-border/70">
          {metrics.map(([label, value]) => (
            <div key={label} className="bg-card p-3">
              <p className="text-[11px] uppercase tracking-wide text-muted-foreground">
                {label}
              </p>
              <p className="mt-1 text-base font-semibold tabular-nums">
                {typeof value === "number"
                  ? label === "Market cap"
                    ? compact(value)
                    : money(value)
                  : "—"}
              </p>
            </div>
          ))}
        </div>
        <p className="line-clamp-6 text-xs leading-5 text-foreground/70">
          {typeof summary === "string" && summary
            ? summary
            : `Company reference data for ${symbol}.`}
        </p>
      </CardContent>
    </Card>
  );
}

function News({ symbol }: { symbol: string }) {
  const { data = [], isLoading, error } = useMarketNews(symbol);
  const { openExternal } = useTradstryPlatform();
  if (isLoading)
    return (
      <div className="grid gap-3 md:grid-cols-2">
        {["news-a", "news-b", "news-c", "news-d", "news-e", "news-f"].map(
          (key) => (
            <Skeleton key={key} className="h-28" />
          ),
        )}
      </div>
    );
  if (error) return <ErrorState message={error.message} />;
  if (!data.length)
    return (
      <div className="flex min-h-64 items-center justify-center rounded-xl border border-dashed text-sm text-muted-foreground">
        No recent stories for {symbol}.
      </div>
    );
  return (
    <div className="grid gap-3 md:grid-cols-2 xl:grid-cols-3">
      {data.slice(0, 18).map((article) => (
        <button
          key={article.url}
          type="button"
          onClick={() => void openExternal(article.url)}
          className="group overflow-hidden rounded-xl border border-border/70 bg-card text-left transition-all hover:-translate-y-0.5 hover:border-foreground/20 hover:shadow-sm"
        >
          {article.imageUrl ? (
            <div className="h-28 overflow-hidden bg-muted">
              <img
                src={article.imageUrl}
                alt=""
                loading="lazy"
                className="size-full object-cover transition-transform duration-300 group-hover:scale-[1.03]"
              />
            </div>
          ) : null}
          <div className="p-4">
            <div className="mb-2 flex items-center justify-between gap-2 text-[11px] text-muted-foreground">
              <span className="truncate font-medium">{article.source}</span>
              <span className="shrink-0">{shortDate(article.publishedAt)}</span>
            </div>
            <h3 className="line-clamp-3 text-sm font-medium leading-snug">
              {article.title}
            </h3>
          </div>
        </button>
      ))}
    </div>
  );
}

function Transcripts({ symbol }: { symbol: string }) {
  const list = useMarketTranscriptList(symbol);
  const { openExternal } = useTradstryPlatform();
  const [selected, setSelected] = React.useState<MarketTranscriptRef | null>(
    null,
  );
  React.useEffect(() => {
    if (!selected && list.data?.[0]) setSelected(list.data[0]);
  }, [list.data, selected]);
  const transcript = useMarketTranscript(
    symbol,
    selected?.quarter,
    selected?.year,
  );
  const sourceUrl = transcript.data?.sourceUrl;
  if (list.isLoading) return <Skeleton className="h-80 w-full" />;
  if (list.error) return <ErrorState message={list.error.message} />;
  if (!list.data?.length)
    return (
      <div className="flex min-h-64 items-center justify-center rounded-xl border border-dashed text-sm text-muted-foreground">
        No earnings transcripts are available for {symbol}.
      </div>
    );
  return (
    <div className="grid h-[calc(100svh-230px)] min-h-[500px] gap-3 lg:grid-cols-[220px_minmax(0,1fr)]">
      <ScrollArea className="h-full rounded-xl border border-border/70 bg-muted/15">
        <div className="space-y-1 p-2">
          {list.data.map((item) => (
            <button
              key={`${item.year}-${item.quarter}`}
              type="button"
              onClick={() => setSelected(item)}
              className={cn(
                "w-full rounded-lg px-3 py-2.5 text-left transition-colors",
                selected?.year === item.year &&
                  selected.quarter === item.quarter
                  ? "bg-primary/10 text-primary ring-1 ring-primary/15"
                  : "hover:bg-muted",
              )}
            >
              <span className="block text-sm font-semibold">
                Q{item.quarter} {item.year}
              </span>
              <span className="mt-0.5 block text-[11px] text-muted-foreground">
                {shortDate(item.date)}
              </span>
            </button>
          ))}
        </div>
      </ScrollArea>
      <Card className="h-full min-w-0 gap-0 py-0">
        <CardHeader className="flex flex-row items-center justify-between border-b py-4">
          <div>
            <CardTitle>
              {selected
                ? `Q${selected.quarter} ${selected.year} earnings call`
                : "Earnings transcript"}
            </CardTitle>
            <p className="mt-1 text-xs text-muted-foreground">
              {selected ? `${symbol} · ${shortDate(selected.date)}` : symbol}
            </p>
          </div>
          {sourceUrl ? (
            <Button
              variant="ghost"
              size="sm"
              onClick={() => void openExternal(sourceUrl)}
            >
              Source
              <HugeiconsIcon icon={ArrowUpRight01Icon} className="size-3.5" />
            </Button>
          ) : null}
        </CardHeader>
        <CardContent className="min-h-0 flex-1 p-0">
          <ScrollArea className="h-full">
            <div className="mx-auto max-w-[78ch] p-5 md:p-7">
              {transcript.isLoading ? (
                <Skeleton className="h-96" />
              ) : transcript.error ? (
                <ErrorState message={transcript.error.message} />
              ) : (
                <div className="whitespace-pre-wrap text-sm leading-7 text-foreground/80">
                  {transcript.data?.content || "Select a transcript."}
                </div>
              )}
            </div>
          </ScrollArea>
        </CardContent>
      </Card>
    </div>
  );
}

function Reports({
  symbol,
  workspaceId,
}: {
  symbol: string;
  workspaceId: string | null;
}) {
  const reports = useMarketReports(workspaceId);
  const generate = useGenerateMarketReport(workspaceId);
  const [focus, setFocus] = React.useState("");
  const [selectedId, setSelectedId] = React.useState<string | null>(null);
  const symbolReports = (reports.data ?? []).filter(
    (item) => item.symbol === symbol,
  );
  const selected =
    symbolReports.find((item) => item.id === selectedId) ?? symbolReports[0];
  return (
    <div className="grid h-[calc(100svh-230px)] min-h-[500px] gap-3 lg:grid-cols-[260px_minmax(0,1fr)]">
      <div className="flex min-h-0 flex-col gap-3">
        <Card size="sm">
          <CardHeader>
            <CardTitle>New research brief</CardTitle>
          </CardHeader>
          <CardContent className="space-y-3">
            <textarea
              value={focus}
              onChange={(e) => setFocus(e.target.value)}
              placeholder="Focus on valuation, earnings, risks…"
              className="min-h-20 w-full resize-none rounded-lg border border-input bg-transparent p-3 text-sm outline-none focus:ring-2 focus:ring-ring"
            />
            <Button
              className="w-full"
              disabled={!workspaceId || generate.isPending}
              onClick={() =>
                generate.mutate(
                  { symbol, focus: focus || undefined },
                  { onSuccess: (report) => setSelectedId(report.id) },
                )
              }
            >
              <HugeiconsIcon icon={SparklesIcon} className="size-4" />
              {generate.isPending ? "Researching…" : `Research ${symbol}`}
            </Button>
            {generate.error ? (
              <p className="text-xs text-destructive">
                {generate.error.message}
              </p>
            ) : null}
          </CardContent>
        </Card>
        <ScrollArea className="min-h-0 flex-1 rounded-xl border border-border/70 bg-muted/15">
          <div className="space-y-1 p-2">
            {symbolReports.length ? (
              symbolReports.map((report) => (
                <button
                  key={report.id}
                  type="button"
                  onClick={() => setSelectedId(report.id)}
                  className={cn(
                    "w-full rounded-lg p-3 text-left transition-colors",
                    selected?.id === report.id
                      ? "bg-primary/10 text-primary ring-1 ring-primary/15"
                      : "hover:bg-muted",
                  )}
                >
                  <span className="line-clamp-2 block text-sm font-medium">
                    {report.title}
                  </span>
                  <span className="mt-1 block text-[11px] text-muted-foreground">
                    {shortDate(report.createdAt)}
                  </span>
                </button>
              ))
            ) : (
              <p className="p-4 text-center text-xs text-muted-foreground">
                No {symbol} briefs yet.
              </p>
            )}
          </div>
        </ScrollArea>
      </div>
      {selected ? (
        <ReportReader report={selected} />
      ) : (
        <Card className="h-full items-center justify-center border-dashed bg-muted/10 text-center">
          <div className="max-w-xs px-6">
            <HugeiconsIcon
              icon={SparklesIcon}
              className="mx-auto mb-3 size-5 text-muted-foreground"
            />
            <p className="text-sm font-medium">No research brief selected</p>
            <p className="mt-1 text-xs text-muted-foreground">
              Generate a cited brief for {symbol} using the company, market,
              news, and transcript data already here.
            </p>
          </div>
        </Card>
      )}
    </div>
  );
}

function ReportReader({ report }: { report: MarketReport }) {
  const { openExternal } = useTradstryPlatform();
  return (
    <Card className="h-full min-w-0 gap-0 py-0">
      <CardHeader className="border-b py-4">
        <CardTitle className="line-clamp-2">{report.title}</CardTitle>
        <p className="text-xs text-muted-foreground">
          {report.symbol} · {shortDate(report.createdAt)}
        </p>
      </CardHeader>
      <CardContent className="min-h-0 flex-1 p-0">
        <ScrollArea className="h-full">
          <div className="mx-auto max-w-[78ch] p-5 md:p-7">
            <div className="space-y-3 text-sm leading-7 [&_h1]:text-xl [&_h1]:font-semibold [&_h2]:pt-4 [&_h2]:text-base [&_h2]:font-semibold [&_li]:ml-5 [&_li]:list-disc [&_p]:text-foreground/80">
              <ReactMarkdown>{report.body}</ReactMarkdown>
            </div>
            {report.sources.length ? (
              <div className="mt-8 border-t pt-4">
                <h3 className="mb-2 text-xs font-semibold uppercase tracking-wider text-muted-foreground">
                  Sources
                </h3>
                <div className="space-y-1">
                  {report.sources.map((source, i) => (
                    <button
                      key={source}
                      type="button"
                      onClick={() => void openExternal(source)}
                      className="block max-w-full truncate text-left text-xs text-blue-600 hover:underline"
                    >
                      [{i + 1}] {source}
                    </button>
                  ))}
                </div>
              </div>
            ) : null}
          </div>
        </ScrollArea>
      </CardContent>
    </Card>
  );
}

function MonitorPanel({
  symbol,
  workspaceId,
}: {
  symbol: string;
  workspaceId: string | null;
}) {
  const monitors = useMarketMonitors(workspaceId);
  const create = useCreateMarketMonitor(workspaceId);
  const remove = useDeleteMarketMonitor(workspaceId);
  const evaluate = useEvaluateMarketMonitors(workspaceId);
  const [threshold, setThreshold] = React.useState("");
  const [condition, setCondition] = React.useState<"ABOVE" | "BELOW">("ABOVE");
  const submit = () => {
    const value = Number(threshold);
    if (!Number.isFinite(value) || value <= 0) return;
    if (
      typeof Notification !== "undefined" &&
      Notification.permission === "default"
    )
      void Notification.requestPermission();
    create.mutate(
      {
        symbol,
        name: `${symbol} ${condition.toLowerCase()} ${money(value)}`,
        condition,
        threshold: value,
      },
      { onSuccess: () => setThreshold("") },
    );
  };
  return (
    <div className="space-y-2.5">
      <div className="grid grid-cols-[minmax(0,1fr)_76px] gap-2">
        <Input
          value={threshold}
          onChange={(e) => setThreshold(e.target.value)}
          inputMode="decimal"
          placeholder="Target price"
          className="h-8 text-xs"
        />
        <select
          value={condition}
          onChange={(e) => setCondition(e.target.value as "ABOVE" | "BELOW")}
          className="h-8 rounded-md border border-input bg-background px-2 text-xs"
        >
          <option value="ABOVE">Above</option>
          <option value="BELOW">Below</option>
        </select>
      </div>
      <Button
        size="sm"
        className="w-full"
        onClick={submit}
        disabled={create.isPending}
      >
        <HugeiconsIcon icon={Notification01Icon} className="size-3.5" />
        Create alert
      </Button>
      <div className="max-h-28 space-y-1.5 overflow-y-auto">
        {(monitors.data ?? []).map((item) => (
          <div
            key={item.id}
            className="flex items-start gap-2 rounded-lg bg-muted/50 p-2"
          >
            <span className="mt-1 size-2 rounded-full bg-emerald-500" />
            <div className="min-w-0 flex-1">
              <p className="truncate text-xs font-medium">{item.name}</p>
              <p className="text-[11px] text-muted-foreground">
                {item.lastTriggeredAt
                  ? `Last triggered ${new Date(item.lastTriggeredAt).toLocaleString()}`
                  : "Watching live price"}
              </p>
            </div>
            <button
              type="button"
              aria-label="Delete monitor"
              onClick={() => remove.mutate(item.id)}
              className="text-muted-foreground hover:text-destructive"
            >
              <HugeiconsIcon icon={Delete02Icon} className="size-4" />
            </button>
          </div>
        ))}
        {!monitors.isLoading && !monitors.data?.length ? (
          <p className="py-1 text-center text-[11px] text-muted-foreground">
            No alerts for {symbol}
          </p>
        ) : null}
      </div>
      <Button
        variant="ghost"
        size="sm"
        className="h-7 w-full text-[11px] text-muted-foreground"
        onClick={() => evaluate.mutate()}
        disabled={evaluate.isPending}
      >
        Check alerts now
      </Button>
    </div>
  );
}

export function Markets() {
  const workspace = useActiveWorkspace();
  const workspaceId = workspace?.id ?? null;
  const watchlists = useMarketWatchlists(workspaceId);
  const createWatchlist = useCreateMarketWatchlist(workspaceId);
  const addSymbol = useAddMarketWatchlistSymbol(workspaceId);
  const removeSymbol = useRemoveMarketWatchlistSymbol(workspaceId);
  const [selectedSymbol, setSelectedSymbol] = React.useState("AAPL");
  const [searchText, setSearchText] = React.useState("");
  const [showSearch, setShowSearch] = React.useState(false);
  const search = useMarketSearch(searchText);
  const symbols = React.useMemo(
    () =>
      Array.from(
        new Set([
          ...(watchlists.data?.flatMap((list) => list.symbols) ?? []),
          ...DEFAULT_SYMBOLS,
        ]),
      ),
    [watchlists.data],
  );
  const quotes = useMarketQuotes(symbols);
  const selectedQuote = quotes.data?.quotes.find(
    (quote) => quote.symbol === selectedSymbol,
  );
  const chat = useChatStore();
  React.useEffect(
    () => () => {
      const state = useChatStore.getState();
      if (state.pinnedContext.marketSymbol === selectedSymbol) {
        state.setPinnedContext({
          ...state.pinnedContext,
          marketSymbol: undefined,
        });
      }
    },
    [selectedSymbol],
  );
  const ensuredWatchlist = React.useRef(false);
  React.useEffect(() => {
    if (
      watchlists.data &&
      watchlists.data.length === 0 &&
      !ensuredWatchlist.current
    ) {
      ensuredWatchlist.current = true;
      createWatchlist.mutate("Core watchlist");
    }
  }, [watchlists.data, createWatchlist]);
  const activeWatchlist = watchlists.data?.[0];

  function askAi() {
    chat.setPinnedContext({
      ...chat.pinnedContext,
      marketSymbol: selectedSymbol,
    });
    chat.setOpen(true);
  }

  return (
    <div className="flex min-h-0 flex-1 overflow-hidden">
      <aside className="hidden w-64 shrink-0 border-r border-border/60 bg-muted/[0.18] lg:flex lg:flex-col">
        <div className="border-b border-border/60 p-3">
          {showSearch ? (
            <div>
              <div className="flex gap-1.5">
                <div className="relative min-w-0 flex-1">
                  <HugeiconsIcon
                    icon={Search01Icon}
                    className="pointer-events-none absolute top-1/2 left-2.5 size-3.5 -translate-y-1/2 text-muted-foreground"
                  />
                  <Input
                    autoFocus
                    value={searchText}
                    onChange={(event) => setSearchText(event.target.value)}
                    placeholder="Search symbols"
                    className="h-8 pl-8 text-xs"
                  />
                </div>
                <Button
                  variant="ghost"
                  size="icon-sm"
                  aria-label="Close symbol search"
                  onClick={() => {
                    setShowSearch(false);
                    setSearchText("");
                  }}
                >
                  <HugeiconsIcon icon={Cancel01Icon} className="size-4" />
                </Button>
              </div>
              {searchText.trim().length >= 2 ? (
                <div className="mt-2 max-h-56 space-y-1 overflow-y-auto">
                  {(search.data ?? []).map((result) => (
                    <button
                      key={result.symbol}
                      type="button"
                      onClick={() => {
                        setSelectedSymbol(result.symbol);
                        setShowSearch(false);
                        setSearchText("");
                        if (activeWatchlist)
                          addSymbol.mutate({
                            watchlistId: activeWatchlist.id,
                            symbol: result.symbol,
                          });
                      }}
                      className="w-full rounded-lg px-2.5 py-2 text-left hover:bg-muted"
                    >
                      <span className="block text-xs font-semibold">
                        {result.symbol}
                      </span>
                      <span className="block truncate text-[11px] text-muted-foreground">
                        {result.name}
                      </span>
                    </button>
                  ))}
                </div>
              ) : null}
            </div>
          ) : (
            <Button
              variant="outline"
              size="sm"
              className="w-full justify-start bg-background"
              onClick={() => setShowSearch(true)}
            >
              <HugeiconsIcon icon={Search01Icon} className="size-4" />
              Find a symbol
            </Button>
          )}
        </div>
        <ScrollArea className="min-h-0 flex-1">
          <div className="p-2.5">
            <p className="px-2 py-2 text-[10px] font-semibold uppercase tracking-[0.12em] text-muted-foreground">
              {activeWatchlist?.name ?? "Watchlist"}
            </p>
            {symbols.map((item) => {
              const quote = quotes.data?.quotes.find(
                (value) => value.symbol === item,
              );
              const selected = selectedSymbol === item;
              return (
                <div
                  key={item}
                  className={cn(
                    "group mb-0.5 flex w-full items-center rounded-lg transition-colors",
                    selected
                      ? "bg-primary/10 text-foreground ring-1 ring-primary/15"
                      : "hover:bg-muted",
                  )}
                >
                  <button
                    type="button"
                    onClick={() => setSelectedSymbol(item)}
                    className="flex min-w-0 flex-1 items-center gap-2 px-2.5 py-2.5 text-left"
                  >
                    <div className="min-w-0 flex-1">
                      <span className="block text-xs font-semibold">
                        {item}
                      </span>
                      <span
                        className={cn(
                          "block truncate text-[11px]",
                          "text-muted-foreground",
                        )}
                      >
                        {quote?.name ?? "Loading…"}
                      </span>
                    </div>
                    <div className="text-right">
                      <span className="block text-xs tabular-nums">
                        {money(quote?.price)}
                      </span>
                      <span
                        className={cn(
                          "text-[11px] tabular-nums",
                          (quote?.changePercent ?? 0) >= 0
                            ? "text-emerald-500"
                            : "text-rose-500",
                        )}
                      >
                        {quote?.changePercent == null
                          ? "—"
                          : `${quote.changePercent >= 0 ? "+" : ""}${quote.changePercent.toFixed(2)}%`}
                      </span>
                    </div>
                  </button>
                  {activeWatchlist?.symbols.includes(item) ? (
                    <button
                      type="button"
                      aria-label={`Remove ${item}`}
                      onClick={() => {
                        removeSymbol.mutate({
                          watchlistId: activeWatchlist.id,
                          symbol: item,
                        });
                      }}
                      className="mr-1 hidden rounded p-1 text-muted-foreground group-hover:block hover:bg-background hover:text-foreground"
                    >
                      <HugeiconsIcon icon={Cancel01Icon} className="size-3" />
                    </button>
                  ) : null}
                </div>
              );
            })}
          </div>
        </ScrollArea>
        <div className="border-t border-border/60 bg-background/70 p-3">
          <p className="mb-2 text-[10px] font-semibold uppercase tracking-[0.12em] text-muted-foreground">
            {selectedSymbol} alerts
          </p>
          <MonitorPanel symbol={selectedSymbol} workspaceId={workspaceId} />
        </div>
      </aside>

      <ScrollArea className="min-h-0 min-w-0 flex-1">
        <div className="mx-auto max-w-[1400px] px-4 pb-6 md:px-6">
          <Tabs defaultValue="overview" className="min-w-0 gap-0">
            <div className="sticky top-0 z-20 -mx-4 border-b border-border/60 bg-background/95 px-4 pt-4 backdrop-blur md:-mx-6 md:px-6">
              <div className="flex flex-wrap items-center justify-between gap-3 pb-3">
                <div className="min-w-0">
                  <div className="flex min-w-0 items-center gap-2.5">
                    <h2 className="text-xl font-semibold tracking-tight">
                      {selectedSymbol}
                    </h2>
                    <span className="truncate text-sm text-muted-foreground">
                      {selectedQuote?.name}
                    </span>
                    {selectedQuote?.isStale ? (
                      <span className="rounded-full bg-amber-500/10 px-2 py-0.5 text-[10px] font-medium text-amber-700">
                        Delayed
                      </span>
                    ) : (
                      <span className="rounded-full bg-emerald-500/10 px-2 py-0.5 text-[10px] font-medium text-emerald-700">
                        Live
                      </span>
                    )}
                  </div>
                  <div className="mt-1 flex flex-wrap items-baseline gap-x-2.5 gap-y-1">
                    <span className="text-3xl font-semibold tracking-tight tabular-nums">
                      {money(selectedQuote?.price)}
                    </span>
                    <span
                      className={cn(
                        "text-sm font-medium tabular-nums",
                        (selectedQuote?.change ?? 0) >= 0
                          ? "text-emerald-600"
                          : "text-rose-600",
                      )}
                    >
                      {selectedQuote?.change == null
                        ? "—"
                        : `${selectedQuote.change >= 0 ? "+" : ""}${selectedQuote.change.toFixed(2)} · ${signedPercent(selectedQuote.changePercent)}`}
                    </span>
                  </div>
                  <p className="mt-0.5 text-[11px] text-muted-foreground">
                    {selectedQuote?.exchange ?? "U.S. market"} ·{" "}
                    {selectedQuote?.marketState ?? "Market status unavailable"}
                    {selectedQuote?.marketTime
                      ? ` · Updated ${new Date(selectedQuote.marketTime).toLocaleTimeString([], { hour: "numeric", minute: "2-digit" })}`
                      : ""}
                  </p>
                </div>
                <Button size="sm" onClick={askAi}>
                  <HugeiconsIcon icon={SparklesIcon} className="size-4" />
                  Ask AI
                </Button>
              </div>
              <div className="mb-2 flex gap-1.5 overflow-x-auto lg:hidden">
                <Button
                  variant="outline"
                  size="sm"
                  onClick={() => setShowSearch((value) => !value)}
                >
                  <HugeiconsIcon icon={Add01Icon} className="size-3.5" />
                  Symbol
                </Button>
                {symbols.map((symbol) => (
                  <Button
                    key={symbol}
                    variant={selectedSymbol === symbol ? "secondary" : "ghost"}
                    size="sm"
                    onClick={() => setSelectedSymbol(symbol)}
                  >
                    {symbol}
                  </Button>
                ))}
              </div>
              <TabsList
                variant="line"
                className="max-w-full overflow-x-auto pb-2"
              >
                <TabsTrigger value="overview">Overview</TabsTrigger>
                <TabsTrigger value="financials">Financials</TabsTrigger>
                <TabsTrigger value="news">News</TabsTrigger>
                <TabsTrigger value="transcripts">Transcripts</TabsTrigger>
                <TabsTrigger value="reports">AI Research</TabsTrigger>
              </TabsList>
            </div>
            {showSearch ? (
              <div className="mt-4 rounded-xl border bg-card p-2 lg:hidden">
                <Input
                  autoFocus
                  value={searchText}
                  onChange={(event) => setSearchText(event.target.value)}
                  placeholder="Search companies or symbols…"
                />
                {searchText.trim().length >= 2 ? (
                  <div className="mt-2 grid max-h-52 gap-1 overflow-y-auto sm:grid-cols-2">
                    {(search.data ?? []).map((result) => (
                      <button
                        key={result.symbol}
                        type="button"
                        onClick={() => {
                          setSelectedSymbol(result.symbol);
                          setShowSearch(false);
                          setSearchText("");
                          if (activeWatchlist)
                            addSymbol.mutate({
                              watchlistId: activeWatchlist.id,
                              symbol: result.symbol,
                            });
                        }}
                        className="rounded-lg px-2.5 py-2 text-left hover:bg-muted"
                      >
                        <span className="block text-xs font-semibold">
                          {result.symbol}
                        </span>
                        <span className="block truncate text-[11px] text-muted-foreground">
                          {result.name}
                        </span>
                      </button>
                    ))}
                  </div>
                ) : null}
              </div>
            ) : null}
            <TabsContent value="overview" className="pt-4">
              <div className="grid gap-4 xl:grid-cols-[minmax(0,1.65fr)_minmax(300px,0.8fr)]">
                <ResearchChart symbol={selectedSymbol} />
                <CompanyOverview
                  symbol={selectedSymbol}
                  quote={selectedQuote}
                />
              </div>
            </TabsContent>
            <TabsContent value="financials" className="pt-4">
              <Financials symbol={selectedSymbol} />
            </TabsContent>
            <TabsContent value="news" className="pt-4">
              <News symbol={selectedSymbol} />
            </TabsContent>
            <TabsContent value="transcripts" className="pt-4">
              <Transcripts key={selectedSymbol} symbol={selectedSymbol} />
            </TabsContent>
            <TabsContent value="reports" className="pt-4">
              <Reports symbol={selectedSymbol} workspaceId={workspaceId} />
            </TabsContent>
          </Tabs>
        </div>
      </ScrollArea>
    </div>
  );
}
