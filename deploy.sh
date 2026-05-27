#!/usr/bin/env bash
set -euo pipefail
export PATH="$HOME/.cargo/bin:/usr/local/bin:/usr/bin:/bin:$PATH"
SYSTEMCTL=/usr/bin/systemctl

cd /home/peter/sp

exec 9>/tmp/sp-deploy.lock
flock -n 9 || exit 0

if [ -f deploy.sh ] && ! git ls-files --error-unmatch deploy.sh >/dev/null 2>&1; then
  cp deploy.sh "/tmp/sp-deploy-untracked-$(date +%Y%m%d%H%M%S).sh"
  rm deploy.sh
fi

OLD_HEAD=$(git rev-parse HEAD)
BRANCH=$(git rev-parse --abbrev-ref HEAD)

git pull --ff-only origin "$BRANCH"

NEW_HEAD=$(git rev-parse HEAD)
CHANGED=$(git diff --name-only "$OLD_HEAD" "$NEW_HEAD" || true)

if echo "$CHANGED" | grep -Eq '^(sp_frontend/)'; then
  cd /home/peter/sp/sp_frontend/sp_ts
  npm ci
  npm run dev
  ./copy.sh
  sudo -n "$SYSTEMCTL" restart sp_axum
fi

if echo "$CHANGED" | grep -Eq '^(sp_server/)'; then
  cd /home/peter/sp/sp_server
  cargo build --release

  if SERVER_STATE=$(sudo -n "$SYSTEMCTL" show sp_server --property=ActiveState --value 2>/dev/null); then
    if [ "$SERVER_STATE" != "active" ]; then
      pkill -x siege_perilous 2>/dev/null || true
    fi
    sudo -n "$SYSTEMCTL" restart sp_server
  else
    if systemctl is-active --quiet sp_server; then
      echo "sp_server is active under systemd, but sudo is unavailable for restart" >&2
      exit 1
    fi
    pkill -x siege_perilous 2>/dev/null || true
    exec 9>&-
    CARGO_MANIFEST_DIR=/home/peter/sp/sp_server nohup ./target/release/siege_perilous > /tmp/sp_server.manual.log 2>&1 < /dev/null &
  fi
fi

if echo "$CHANGED" | grep -Eq '^(sp_axum/)'; then
  cd /home/peter/sp/sp_axum
  cargo build --release
  sudo -n "$SYSTEMCTL" restart sp_axum
fi
