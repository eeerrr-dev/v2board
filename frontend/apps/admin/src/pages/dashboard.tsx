import { useCallback, useEffect, useRef, useState, type MutableRefObject } from 'react';
import { useNavigate } from 'react-router-dom';
import * as echarts from 'echarts';
import 'echarts/theme/vintage';
import type { ECharts, EChartsOption } from 'echarts';
import type { AdminFilter } from '@v2board/api-client';
import type { OrderStatPoint, ServerRankItem, UserRankItem } from '@v2board/types';
import {
  useConfig,
  useStat,
  useStatOrder,
  useStatServerLast,
  useStatServerToday,
  useStatUserLast,
  useStatUserToday,
} from '@/lib/queries';
import { getAdminApiBaseUrl } from '@/lib/legacy-settings';
import { legacyHref } from '@/lib/legacy-href';

type EmptyOrderChartElement = HTMLDivElement & {
  __v2boardEmptyOrderChartCleanup?: () => void;
};

function formatCent(value?: number) {
  return value ? (value / 100).toFixed(2) : '0.00';
}

function useChart<T>(
  data: T[] | undefined,
  render: (element: HTMLDivElement, data: T[]) => ECharts,
  chartObjectRef: MutableRefObject<ECharts | undefined>,
  onRendered?: () => void | (() => void),
) {
  const ref = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!ref.current || !data) return undefined;
    chartObjectRef.current = render(ref.current, data);
    const cleanup = onRendered?.();
    return () => {
      if (typeof cleanup === 'function') cleanup();
    };
  }, [chartObjectRef, data, onRendered, render]);

  return ref;
}

function clearEmptyOrderChartBaseline(element: EmptyOrderChartElement) {
  element.__v2boardEmptyOrderChartCleanup?.();
  delete element.__v2boardEmptyOrderChartCleanup;
  element.querySelector('.v2board-empty-order-chart-canvas')?.remove();
}

function renderEmptyOrderChartBaseline(element: EmptyOrderChartElement) {
  clearEmptyOrderChartBaseline(element);

  const canvas = document.createElement('canvas');
  canvas.className = 'v2board-empty-order-chart-canvas';
  element.append(canvas);

  const paint = () => {
    const width = Math.max(1, Math.round(element.clientWidth - 6));
    const scale = window.devicePixelRatio || 1;
    canvas.width = width * scale;
    canvas.height = scale;

    const context = canvas.getContext('2d');
    if (!context) return;

    context.setTransform(scale, 0, 0, scale, 0, 0);
    context.clearRect(0, 0, width, 1);
    context.fillStyle = document.documentElement.dataset.darkreaderMode
      ? 'rgb(160, 152, 137)'
      : '#6e7079';
    context.fillRect(0, 0, width, 1);
  };

  const observer = new MutationObserver(paint);
  observer.observe(document.documentElement, {
    attributeFilter: ['data-darkreader-mode', 'data-darkreader-scheme'],
    attributes: true,
  });

  window.addEventListener('resize', paint);
  element.__v2boardEmptyOrderChartCleanup = () => {
    observer.disconnect();
    window.removeEventListener('resize', paint);
  };

  paint();
  requestAnimationFrame(paint);
}

function renderOrderChart(element: HTMLDivElement, data: OrderStatPoint[]) {
  const chart = echarts.init(element, 'vintage', { renderer: 'svg' });
  if (data.length === 0) {
    element.classList.add('v2board-empty-order-chart');
    element.style.position = '';
    renderEmptyOrderChartBaseline(element);
    return chart;
  }
  element.classList.remove('v2board-empty-order-chart');
  clearEmptyOrderChartBaseline(element);

  const option: EChartsOption & {
    legend: { data: string[]; left: string; z: number };
    xAxis: {
      boundaryGap: boolean;
      data: string[];
      type: string;
    };
    series: { name: string; type: 'line'; smooth: boolean; data: number[] }[];
  } = {
    tooltip: { trigger: 'axis' },
    legend: { data: [], left: '0', z: 4 },
    grid: { left: '1%', right: '1%', bottom: '3%', containLabel: true },
    xAxis: {
      type: 'category',
      boundaryGap: false,
      data: [],
    },
    yAxis: { type: 'value' },
    series: [],
  };

  data.forEach((point) => {
    if (!option.legend.data.includes(point.type)) option.legend.data.push(point.type);
    if (!option.xAxis.data.includes(point.date)) option.xAxis.data.push(point.date);
    const series = option.series.find((item) => item.name === point.type);
    if (series) {
      series.data.push(point.value);
    } else {
      option.series.push({
        name: point.type,
        type: 'line',
        smooth: true,
        data: [point.value],
      });
    }
  });

  chart.setOption(option);
  element.style.position = '';
  return chart;
}

function renderRankChart<T extends ServerRankItem | UserRankItem>(
  element: HTMLDivElement,
  data: T[],
  getName: (item: T) => string,
) {
  const chart = echarts.init(element);
  const option: EChartsOption & {
    yAxis: { type: string; data: string[] };
    series: { data: number[]; type: 'bar' }[];
  } = {
    tooltip: {
      trigger: 'axis',
      formatter: (params) => `${(params as Array<{ value: unknown }>)[0]!.value} GB`,
    },
    grid: { top: '1%', left: '1%', right: '1%', bottom: '3%', containLabel: true },
    xAxis: { type: 'value' },
    yAxis: { type: 'category', data: [] },
    series: [{ data: [], type: 'bar' }],
  };

  data.reverse().forEach((item) => {
    option.yAxis.data.push(getName(item));
    const series = option.series[0];
    if (series) series.data.push(item.total);
  });

  chart.setOption(option);
  element.style.position = '';
  return chart;
}

function renderServerRankChart(element: HTMLDivElement, data: ServerRankItem[]) {
  return renderRankChart(element, data, (item) => item.server_name);
}

function renderUserRankChart(element: HTMLDivElement, data: UserRankItem[]) {
  return renderRankChart(element, data, (item) => item.email);
}

function useQueueStatus() {
  const [queueStatus, setQueueStatus] = useState<string>();

  useEffect(() => {
    const origin = new URL(getAdminApiBaseUrl()).origin;
    fetch(`${origin}/monitor/api/stats`)
      .then((response) => response.json())
      .then((data) => setQueueStatus(data?.status));
  }, []);

  return queueStatus;
}

const PENDING_COMMISSION_ORDER_FILTER: AdminFilter[] = [
  { key: 'status', condition: '=', value: '3' },
  { key: 'commission_status', condition: '=', value: '0' },
  { key: 'commission_balance', condition: '>', value: '0' },
];

export default function DashboardPage() {
  const navigate = useNavigate();
  const queueStatus = useQueueStatus();
  const stat = useStat();
  const order = useStatOrder();
  const serverLast = useStatServerLast();
  const serverToday = useStatServerToday();
  const userToday = useStatUserToday();
  const userLast = useStatUserLast();
  const config = useConfig('site');
  const currency = config.data?.site?.currency;
  const data = stat.data;
  const orderChartObj = useRef<ECharts | undefined>(undefined);
  const serverLastRankChartObj = useRef<ECharts | undefined>(undefined);
  const serverTodayRankChartObj = useRef<ECharts | undefined>(undefined);
  const userTodayRankChartObj = useRef<ECharts | undefined>(undefined);
  const userLastRankChartObj = useRef<ECharts | undefined>(undefined);

  const goPendingCommissionOrders = () => {
    window.sessionStorage.setItem(
      'v2board-admin-order-filter',
      JSON.stringify(PENDING_COMMISSION_ORDER_FILTER),
    );
    navigate('/order');
  };

  const resizeAllCharts = useCallback(() => {
    orderChartObj.current!.resize();
    serverLastRankChartObj.current!.resize();
    serverTodayRankChartObj.current!.resize();
    userTodayRankChartObj.current!.resize();
    userLastRankChartObj.current!.resize();
  }, []);
  const registerLegacyOrderChartResize = useCallback(() => {
    window.addEventListener('resize', resizeAllCharts);
    return () => {
      // The bundled class component removes `this.chartResize.bind(this)`,
      // which is not the listener it added. Keep that no-op removal shape.
      window.removeEventListener('resize', () => resizeAllCharts());
    };
  }, [resizeAllCharts]);

  const orderChart = useChart(
    order.data,
    renderOrderChart,
    orderChartObj,
    registerLegacyOrderChartResize,
  );
  const serverLastRankChart = useChart(
    serverLast.data,
    renderServerRankChart,
    serverLastRankChartObj,
  );
  const serverTodayRankChart = useChart(
    serverToday.data,
    renderServerRankChart,
    serverTodayRankChartObj,
  );
  const userTodayRankChart = useChart(userToday.data, renderUserRankChart, userTodayRankChartObj);
  const userLastRankChart = useChart(userLast.data, renderUserRankChart, userLastRankChartObj);

  return (
    <>
      {queueStatus && queueStatus !== 'running' ? (
        <div className="row">
          <div className="col-lg-12">
            <div className="alert alert-danger" role="alert">
              <p className="mb-0">当前队列服务运行异常，可能会导致业务无法使用。</p>
            </div>
          </div>
        </div>
      ) : null}

      {data?.ticket_pending_total ? (
        <div className="alert alert-danger" role="alert">
          <p className="mb-0">
            有 {data.ticket_pending_total} 条工单等待处理{' '}
            <a
              className="alert-link"
              ref={legacyHref('javascript:void(0)')}
              onClick={() => navigate('/ticket')}
            >
              立即处理
            </a>
          </p>
        </div>
      ) : null}

      {data?.commission_pending_total ? (
        <div className="alert alert-danger" role="alert">
          <p className="mb-0">
            有 {data.commission_pending_total} 笔佣金等待确认{' '}
            <a
              className="alert-link"
              ref={legacyHref('javascript:void(0)')}
              onClick={goPendingCommissionOrders}
            >
              立即处理
            </a>
          </p>
        </div>
      ) : null}

      <div className="mb-0 block border-bottom js-classic-nav d-none d-sm-block">
        <div className="block-content block-content-full">
          <div className="row no-gutters border">
            <div className="col-sm-6 col-xl-3 js-appear-enabled animated" data-toggle="appear">
              <a
                className="block block-bordered block-link-pop text-center mb-0"
                onClick={() => navigate('/config/system')}
              >
                <div className="block-content block-content-full text-center">
                  <i className="fa-2x si si-equalizer text-primary d-none d-sm-inline-block mb-3" />
                  <div className="font-w600 text-uppercase">系统设置</div>
                </div>
              </a>
            </div>
            <div className="col-sm-6 col-xl-3 js-appear-enabled animated" data-toggle="appear">
              <a
                className="block block-bordered block-link-pop text-center mb-0"
                onClick={() => navigate('/order')}
              >
                <div className="block-content block-content-full text-center">
                  <i className="fa-2x si si-list text-primary d-none d-sm-inline-block mb-3" />
                  <div className="font-w600 text-uppercase">订单管理</div>
                </div>
              </a>
            </div>
            <div className="col-sm-6 col-xl-3 js-appear-enabled animated" data-toggle="appear">
              <a
                className="block block-bordered block-link-pop text-center mb-0"
                onClick={() => navigate('/plan')}
              >
                <div className="block-content block-content-full text-center">
                  <i className="fa-2x si si-bag text-primary d-none d-sm-inline-block mb-3" />
                  <div className="font-w600 text-uppercase">订阅管理</div>
                </div>
              </a>
            </div>
            <div className="col-sm-6 col-xl-3 js-appear-enabled animated" data-toggle="appear">
              <a
                className="block block-bordered block-link-pop text-center mb-0"
                onClick={() => navigate('/user')}
              >
                <div className="block-content block-content-full text-center">
                  <i className="fa-2x si si-users text-primary d-none d-sm-inline-block mb-3" />
                  <div className="font-w600 text-uppercase">用户管理</div>
                </div>
              </a>
            </div>
          </div>
        </div>
      </div>

      <div className="row no-gutters">
        <div className="col-lg-12 js-appear-enabled animated" data-toggle="appear">
          <div className="block border-bottom mb-0 v2board-stats-bar">
            <div className="block-content">
              <div className="d-flex align-items-center">
                <div className="pr-4 pr-sm-5 pl-0 pl-sm-3 ">
                  <i className="fa fa-users fa-2x text-gray-light float-right" />
                  <div className="text-muted mb-1" style={{ width: '120px' }}>
                    在线人数
                  </div>
                  <div className="display-4 text-black font-w300 mb-2">
                    {data?.online_user ? data.online_user : '0'}
                  </div>
                </div>
                <div className="pr-4 pr-sm-5 pl-0 pl-sm-3 ">
                  <i className="fa fa-chart-line fa-2x text-gray-light float-right" />
                  <p className="text-muted w-75 mb-1">今日收入</p>
                  <p className="display-4 text-black font-w300 mb-2">
                    {formatCent(data?.day_income)}
                    <span className="font-size-h5 font-w600 text-muted">{currency}</span>
                  </p>
                </div>
                <div className="pr-4 pr-sm-5 pl-0 pl-sm-3 ">
                  <i className="fa fa-user fa-2x text-gray-light float-right" />
                  <div className="text-muted mb-1" style={{ width: '120px' }}>
                    实时注册
                  </div>
                  <div className="display-4 text-black font-w300 mb-2">
                    {data?.day_register_total ? data.day_register_total : '0'}
                  </div>
                </div>
              </div>
            </div>
          </div>
        </div>

        <div className="col-lg-12 js-appear-enabled animated" data-toggle="appear">
          <div
            className="block border-bottom mb-0 v2board-stats-bar"
            onScroll={(event) => console.log(event.currentTarget.scrollLeft)}
          >
            <div className="block-content block-content-full">
              <div className="d-flex align-items-center">
                <div className="pr-4 pr-sm-5 pl-0 pl-sm-3">
                  <p className="fs-3 text-dark mb-0">
                    {formatCent(data?.month_income)} {currency}
                  </p>
                  <p className="text-muted mb-0">本月收入</p>
                </div>
                <div className="px-4 px-sm-5 border-start">
                  <p className="fs-3 text-dark mb-0">
                    {formatCent(data?.last_month_income)} {currency}
                  </p>
                  <p className="text-muted mb-0">上月收入</p>
                </div>
                <div className="px-4 px-sm-5 border-start">
                  <p className="fs-3 text-dark mb-0">
                    {formatCent(data?.commission_last_month_payout)} {currency}
                  </p>
                  <p className="text-muted mb-0">上月佣金支出</p>
                </div>
                <div className="px-4 px-sm-5 border-start">
                  <p className="fs-3 text-dark mb-0">{data?.month_register_total || '-'}</p>
                  <p className="text-muted mb-0">本月新增用户</p>
                </div>
              </div>
            </div>
          </div>
        </div>

        <div className="col-lg-12 js-appear-enabled animated" data-toggle="appear">
          <div className="block border-bottom mb-0">
            <div
              className="px-sm-3 pt-sm-3 py-3 clearfix"
              id="orderChart"
              style={{ height: 400 }}
              ref={orderChart}
            />
          </div>
        </div>

        <div className="row mt-xl-3">
          <div className="col-lg-6 js-appear-enabled animated pr-xl-1" data-toggle="appear">
            <div className="block border-bottom">
              <div className="block-header block-header-default">
                <h3 className="block-title">今日节点流量排行</h3>
              </div>
              <div className="block-content">
                <div
                  className="px-sm-3 pt-sm-3 py-3 clearfix"
                  id="serverTodayRankChart"
                  style={{ height: 400 }}
                  ref={serverTodayRankChart}
                />
              </div>
            </div>
          </div>
          <div className="col-lg-6 js-appear-enabled animated" data-toggle="appear">
            <div className="block border-bottom">
              <div className="block-header block-header-default">
                <h3 className="block-title">昨日节点流量排行</h3>
              </div>
              <div className="block-content">
                <div
                  className="px-sm-3 pt-sm-3 py-3 clearfix"
                  id="serverLastRankChart"
                  style={{ height: 400 }}
                  ref={serverLastRankChart}
                />
              </div>
            </div>
          </div>
          <div className="col-lg-6 js-appear-enabled animated pr-xl-1" data-toggle="appear">
            <div className="block border-bottom">
              <div className="block-header block-header-default">
                <h3 className="block-title">今日用户流量排行</h3>
              </div>
              <div className="block-content">
                <div
                  className="px-sm-3 pt-sm-3 py-3 clearfix"
                  id="userTodayRankChart"
                  style={{ height: 400 }}
                  ref={userTodayRankChart}
                />
              </div>
            </div>
          </div>
          <div className="col-lg-6 js-appear-enabled animated" data-toggle="appear">
            <div className="block border-bottom">
              <div className="block-header block-header-default">
                <h3 className="block-title">昨日用户流量排行</h3>
              </div>
              <div className="block-content">
                <div
                  className="px-sm-3 pt-sm-3 py-3 clearfix"
                  id="userLastRankChart"
                  style={{ height: 400 }}
                  ref={userLastRankChart}
                />
              </div>
            </div>
          </div>
        </div>
      </div>
    </>
  );
}
