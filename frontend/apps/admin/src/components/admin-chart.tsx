import { useMemo } from 'react';
import { getI18n, useTranslation } from 'react-i18next';
import type { SelectorParam } from 'i18next';
import type { StatSeriesPoint } from '@v2board/types';
import { Bar, BarChart, CartesianGrid, Line, LineChart, XAxis, YAxis } from 'recharts';
import { cn } from '@/lib/cn';
import {
  ChartContainer,
  ChartLegend,
  ChartLegendContent,
  ChartTooltip,
  ChartTooltipContent,
  type ChartConfig,
} from '@/components/ui/chart';

// Dashboard loads this module through React.lazy so Recharts stays out of the
// admin entry and the alert/summary-card render path.
const CHART_INITIAL_DIMENSION = { width: 320, height: 360 } as const;
const CHART_COLORS = [
  'var(--chart-1)',
  'var(--chart-2)',
  'var(--chart-3)',
  'var(--chart-4)',
  'var(--chart-5)',
] as const;

export interface RankingChartDatum {
  name: string;
  total: number;
}

interface OrderChartProps {
  kind: 'order';
  data: readonly StatSeriesPoint[];
  label: string;
  className?: string;
}

interface RankingChartProps {
  kind: 'ranking';
  data: readonly RankingChartDatum[];
  label: string;
  className?: string;
}

export type AdminChartProps = OrderChartProps | RankingChartProps;

interface OrderChartSeries {
  dataKey: string;
  label: string;
}

export interface OrderChartRow {
  date: string;
  [key: string]: number | string | undefined;
}

export interface OrderChartModel {
  rows: OrderChartRow[];
  series: OrderChartSeries[];
}

// §6.8 (W14) series re-spec: the wire carries stable snake_case `series`
// slugs with integer-cent money values; the client owns the display mapping
// (slug → label, cents → major units). Slugs outside this map still render,
// labeled by their raw slug, so a backend-added series degrades visibly
// instead of vanishing.
const ORDER_SERIES_DISPLAY: ReadonlyMap<string, { labelKey: SelectorParam; cents?: boolean }> =
  new Map([
    ['register_count', { labelKey: ($) => $.admin.dashboard.series_register_count }],
    ['paid_total', { labelKey: ($) => $.admin.dashboard.series_paid_total, cents: true }],
    ['paid_count', { labelKey: ($) => $.admin.dashboard.series_paid_count }],
    [
      'commission_paid_total',
      { labelKey: ($) => $.admin.dashboard.series_commission_paid_total, cents: true },
    ],
    ['commission_paid_count', { labelKey: ($) => $.admin.dashboard.series_commission_paid_count }],
  ]);

const orderSeriesRank = (slug: string) => {
  const rank = [...ORDER_SERIES_DISPLAY.keys()].indexOf(slug);
  return rank === -1 ? ORDER_SERIES_DISPLAY.size : rank;
};

type TranslateSelector = (selector: SelectorParam) => string;

// The default resolver serves direct (non-component) callers such as unit
// tests; OrderChart passes its provider-bound `t` so labels stay reactive.
const defaultTranslateLabel: TranslateSelector = (selector) => getI18n().t(selector);

export function buildOrderChartModel(
  points: readonly StatSeriesPoint[],
  translateLabel: TranslateSelector = defaultTranslateLabel,
): OrderChartModel {
  const slugs = Array.from(new Set(points.map((point) => point.series))).sort(
    (a, b) => orderSeriesRank(a) - orderSeriesRank(b),
  );
  const series = slugs.map((slug, index) => {
    const display = ORDER_SERIES_DISPLAY.get(slug);
    return {
      dataKey: `series_${index}`,
      label: display ? translateLabel(display.labelKey) : slug,
    };
  });
  const dataKeyBySlug = new Map(slugs.map((slug, index) => [slug, `series_${index}`]));
  const rowsByDate = new Map<string, OrderChartRow>();

  for (const point of points) {
    const row = rowsByDate.get(point.date) ?? { date: point.date };
    const dataKey = dataKeyBySlug.get(point.series);
    if (dataKey) {
      row[dataKey] = ORDER_SERIES_DISPLAY.get(point.series)?.cents
        ? point.value / 100
        : point.value;
    }
    rowsByDate.set(point.date, row);
  }

  return { rows: Array.from(rowsByDate.values()), series };
}

export default function AdminChart(props: AdminChartProps) {
  return props.kind === 'order' ? <OrderChart {...props} /> : <RankingChart {...props} />;
}

function OrderChart({ data, label, className }: OrderChartProps) {
  const { t } = useTranslation();
  // Deliberate useMemo (not compiler-elided): recharts restarts its mount
  // animation when the data/config prop identity changes, so these must stay
  // referentially stable across unrelated parent re-renders.
  const model = useMemo(() => buildOrderChartModel(data, (selector) => t(selector)), [data, t]);
  const config = useMemo<ChartConfig>(
    () =>
      Object.fromEntries(
        model.series.map((series, index) => [
          series.dataKey,
          {
            label: series.label,
            color: CHART_COLORS[index % CHART_COLORS.length]!,
          },
        ]),
      ),
    [model.series],
  );

  return (
    <ChartContainer
      config={config}
      initialDimension={CHART_INITIAL_DIMENSION}
      data-testid="admin-order-chart"
      className={cn('aspect-auto h-[360px] min-h-[360px] w-full min-w-0', className)}
    >
      <LineChart
        accessibilityLayer
        aria-label={label}
        data={model.rows}
        margin={{ top: 4, right: 12, bottom: 4, left: 4 }}
      >
        <CartesianGrid vertical={false} />
        <XAxis dataKey="date" tickLine={false} axisLine={false} tickMargin={8} minTickGap={24} />
        <YAxis tickLine={false} axisLine={false} tickMargin={8} width="auto" />
        <ChartTooltip content={<ChartTooltipContent indicator="line" />} />
        <ChartLegend
          verticalAlign="top"
          align="left"
          content={<ChartLegendContent className="justify-start pt-0" />}
        />
        {model.series.map((series) => (
          <Line
            key={series.dataKey}
            type="monotone"
            dataKey={series.dataKey}
            stroke={`var(--color-${series.dataKey})`}
            strokeWidth={2}
            dot={false}
            activeDot={{ r: 4 }}
            connectNulls={false}
          />
        ))}
      </LineChart>
    </ChartContainer>
  );
}

function RankingChart({ data, label, className }: RankingChartProps) {
  const { t } = useTranslation();
  // Deliberate useMemo: stable rows identity keeps recharts from replaying
  // its mount animation on unrelated parent re-renders.
  const rows = useMemo(() => [...data].reverse(), [data]);
  const config = useMemo<ChartConfig>(
    () => ({
      total: { label: t(($) => $.admin.dashboard.ranking_traffic), color: 'var(--chart-1)' },
    }),
    [t],
  );

  return (
    <ChartContainer
      config={config}
      initialDimension={CHART_INITIAL_DIMENSION}
      data-testid="admin-ranking-chart"
      className={cn('aspect-auto h-[360px] min-h-[360px] w-full min-w-0', className)}
    >
      <BarChart
        accessibilityLayer
        aria-label={label}
        data={rows}
        layout="vertical"
        margin={{ top: 4, right: 24, bottom: 4, left: 4 }}
      >
        <CartesianGrid horizontal={false} />
        <XAxis
          type="number"
          dataKey="total"
          tickLine={false}
          axisLine={false}
          tickMargin={8}
          tickFormatter={(value) => `${Number(value).toLocaleString()} GB`}
        />
        <YAxis
          type="category"
          dataKey="name"
          tickLine={false}
          axisLine={false}
          tickMargin={8}
          width="auto"
        />
        <ChartTooltip
          cursor={false}
          content={
            <ChartTooltipContent
              hideLabel
              valueFormatter={(value) => `${Number(value).toLocaleString()} GB`}
            />
          }
        />
        <Bar dataKey="total" fill="var(--color-total)" radius={[0, 4, 4, 0]} />
      </BarChart>
    </ChartContainer>
  );
}
