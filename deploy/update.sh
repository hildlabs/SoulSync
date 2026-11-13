#!/bin/sh
# Updates the run server whenever main has new commits, and does nothing
# otherwise. Runs survive this since they are kept in the `runs` volume.
#
# With SOULSYNC_TRACK=releases it follows released versions instead (the
# tags v0.1.0, v0.2.0, ...), so a public server only changes when a version
# is released, not with every commit.
#
# Run it from cron. Players are cut off for a few seconds whenever it does
# update, so a quiet hour suits best, for example 4 a.m. every day:
#
#   (crontab -l 2>/dev/null; echo "0 4 * * * $PWD/update.sh >> $HOME/soulsync-update.log 2>&1") | crontab -
#
# For a server others play on, follow released versions:
#
#   0 4 * * * SOULSYNC_TRACK=releases /path/to/update.sh >> ...
#
# It starts the server with the `https` profile (Caddy). Set SOULSYNC_PROFILE
# in the crontab line to change that, or to "" for the server alone.
set -eu
cd "$(dirname "$0")/.."

# One update at a time: a build can take longer than the cron interval.
exec 9>/tmp/soulsync-update.lock
flock -n 9 || exit 0

if [ "${SOULSYNC_TRACK-main}" = releases ]; then
  git fetch -q --tags origin
  target=$(git tag -l 'v*' --sort=-v:refname | head -n 1)
  [ -n "$target" ] || { echo "$(date -Iseconds) no released version yet"; exit 0; }
  [ "$(git rev-parse HEAD)" = "$(git rev-parse "$target^{commit}")" ] && exit 0
  git -c advice.detachedHead=false checkout -q "$target"
else
  git fetch -q origin main
  [ "$(git rev-parse HEAD)" = "$(git rev-parse origin/main)" ] && exit 0
  git merge -q --ff-only origin/main
fi

cd deploy
profile=${SOULSYNC_PROFILE-https}
if [ -n "$profile" ]; then
  docker compose --profile "$profile" up -d --build
else
  docker compose up -d --build
fi
docker image prune -f >/dev/null
echo "$(date -Iseconds) updated to $(git rev-parse --short HEAD)"
