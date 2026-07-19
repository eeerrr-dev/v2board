// Load-smoke for the two cheapest public read paths: the SPA shell and the
// guest public config. This is an operator tool, not a CI gate — run it
// against the local Docker stack after `make up`:
//
//   docker run --rm --network host -v "$PWD/docs/examples:/scripts:ro" \
//     grafana/k6:latest run /scripts/k6-smoke.js
//
// Override the target with -e BASE_URL=... (never point it at production
// through Cloudflare without operator approval; edge rate limiting and WAF
// will skew the numbers anyway).

import http from 'k6/http';
import { check } from 'k6';

const BASE_URL = __ENV.BASE_URL || 'http://localhost:8000';

export const options = {
  vus: 10,
  duration: '30s',
  thresholds: {
    http_req_failed: ['rate<0.01'],
    'http_req_duration{endpoint:shell}': ['p(95)<250'],
    'http_req_duration{endpoint:public_config}': ['p(95)<250'],
  },
};

export default function () {
  const shell = http.get(`${BASE_URL}/`, {
    tags: { endpoint: 'shell' },
  });
  check(shell, {
    'shell responds 200': (r) => r.status === 200,
    'shell is HTML': (r) =>
      (r.headers['Content-Type'] || '').includes('text/html'),
  });

  const config = http.get(`${BASE_URL}/api/v1/public/config`, {
    tags: { endpoint: 'public_config' },
  });
  check(config, {
    'public config responds 200': (r) => r.status === 200,
    'public config is JSON': (r) =>
      (r.headers['Content-Type'] || '').includes('application/json'),
  });
}
