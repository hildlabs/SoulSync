#!/bin/sh
# Updates the run server whenever main has new commits, and does nothing
# otherwise. Runs survive this since they are kept in the `runs` volume.
#
# Run it from cron, for example every 5 minutes:
#
#   (crontab -l 2>/dev/null; echo "*/5 * * * * $PWD/update.sh >> $HOME/soulsync-update.log 2>&1") | crontab -
#
# It starts the server with the `https` profile (Caddy). Set SOULSYNC_PROFILE
# in the crontab line to change that, or to "" for the server alone.
set -eu
cd "$(dirname "$0")/.."

# One update at a time: a build can take longer than the cron interval.
exec 9>/tmp/soulsync-update.lock
flock -n 9 || exit 0

git fetch -q origin main
[ "$(git rev-parse HEAD)" = "$(git rev-parse origin/main)" ] && exit 0
git merge -q --ff-only origin/main

cd deploy
profile=${SOULSYNC_PROFILE-https}
if [ -n "$profile" ]; then
  docker compose --profile "$profile" up -d --build
else
  docker compose up -d --build
fi
docker image prune -f >/dev/null
echo "$(date -Iseconds) updated to $(git rev-parse --short HEAD)"
