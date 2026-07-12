import { useMemo } from 'react';
import type { OrderStatPoint } from '@v2board/types';
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
  data: readonly OrderStatPoint[];
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

export function buildOrderChartModel(points: readonly OrderStatPoint[]): OrderChartModel {
  const labels = Array.from(new Set(points.map((point) => point.type)));
  const series = labels.map((label, index) => ({ dataKey: `series_${index}`, label }));
  const dataKeyByLabel = new Map(series.map((item) => [item.label, item.dataKey]));
  const rowsByDate = new Map<string, OrderChartRow>();

  for (const point of points) {
    const row = rowsByDate.get(point.date) ?? { date: point.date };
    const dataKey = dataKeyByLabel.get(point.type);
    if (dataKey) row[dataKey] = point.value;
    rowsByDate.set(point.date, row);
  }

  return { rows: Array.from(rowsByDate.values()), series };
}

const rankingConfig = {
  total: { label: '流量', color: 'var(--chart-1)' },
} satisfies ChartConfig;

export default function AdminChart(props: AdminChartProps) {
  return props.kind === 'order' ? <OrderChart {...props} /> : <RankingChart {...props} />;
}

function OrderChart({ data, label, className }: OrderChartProps) {
  const model = useMemo(() => buildOrderChartModel(data), [data]);
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
      className={cn('h-[360px] min-h-[360px] min-w-0 w-full aspect-auto', className)}
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
  const rows = useMemo(() => [...data].reverse(), [data]);

  return (
    <ChartContainer
      config={rankingConfig}
      initialDimension={CHART_INITIAL_DIMENSION}
      data-testid="admin-ranking-chart"
      className={cn('h-[360px] min-h-[360px] min-w-0 w-full aspect-auto', className)}
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
