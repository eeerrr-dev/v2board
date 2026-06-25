#!/usr/bin/env bash
set -e

ROLE="${ROLE:-app}"
cd /app

write_env() {
    cat > /app/.env <<'EOF'
APP_NAME=V2Board
APP_ENV=local
APP_KEY=
APP_DEBUG=true
APP_URL=http://localhost:8000

LOG_CHANNEL=stack

DB_CONNECTION=mysql
DB_HOST=mysql
DB_PORT=3306
DB_DATABASE=v2board
DB_USERNAME=v2board
DB_PASSWORD=v2board

BROADCAST_DRIVER=log
CACHE_DRIVER=redis
QUEUE_CONNECTION=redis
SESSION_DRIVER=redis
SESSION_LIFETIME=120

REDIS_HOST=redis
REDIS_PASSWORD=null
REDIS_PORT=6379

MAIL_DRIVER=smtp
MAIL_HOST=mailpit
MAIL_PORT=1025
MAIL_USERNAME=null
MAIL_PASSWORD=null
MAIL_ENCRYPTION=null
MAIL_FROM_ADDRESS=noreply@v2board.local
MAIL_FROM_NAME=V2Board

HORIZON_PREFIX=horizon
EOF
}

wait_for() {
    local host="$1" port="$2" name="$3"
    echo "[entrypoint:${ROLE}] waiting for ${name} (${host}:${port})..."
    until (echo > "/dev/tcp/${host}/${port}") >/dev/null 2>&1; do
        sleep 1
    done
}

if [ "$ROLE" = "app" ]; then
    [ -f /app/.env ] || write_env

    if [ ! -d /app/vendor/laravel ]; then
        echo "[entrypoint:app] composer install..."
        composer install --prefer-dist --no-progress --no-interaction
    fi

    if ! grep -q '^APP_KEY=base64:' /app/.env; then
        echo "[entrypoint:app] generating APP_KEY..."
        php artisan key:generate --force
    fi

    wait_for mysql 3306 mysql
    wait_for redis 6379 redis

    if ! mysql -h mysql -uroot -pv2board v2board -e "SELECT 1 FROM v2_user LIMIT 1" >/dev/null 2>&1; then
        echo "[entrypoint:app] importing schema from database/install.sql..."
        mysql -h mysql -uroot -pv2board v2board < /app/database/install.sql
    fi

    echo "[entrypoint:app] ensuring local seed data + settings..."
    php /app/docker/seed.php

    php artisan config:clear || true
    php artisan view:clear || true

else
    # horizon / scheduler: app container already prepared shared state
    until [ -f /app/.env ] && [ -f /app/vendor/autoload.php ] && [ -f /app/config/v2board.php ]; do
        echo "[entrypoint:${ROLE}] waiting for app bootstrap..."
        sleep 2
    done
    wait_for mysql 3306 mysql
    wait_for redis 6379 redis
fi

echo "[entrypoint:${ROLE}] starting: $*"
exec "$@"
