#!/usr/bin/env bash
SOURCE="${BASH_SOURCE[0]}"
while [ -L "$SOURCE" ]; do # resolve $SOURCE until the file is no longer a symlink
  DIR=$( cd -P "$( dirname "$SOURCE" )" >/dev/null 2>&1 && pwd )
  SOURCE=$(readlink "$SOURCE")
  [[ $SOURCE != /* ]] && SOURCE="$DIR/$SOURCE" # if $SOURCE was a relative symlink, we need to resolve it relative to the path where the symlink file was located
done
DIR=$( cd -P "$( dirname "$SOURCE" )" >/dev/null 2>&1 && pwd )

BIN_LOCATION="$DIR/target/release/locationapp-server"

export GOOGLE_CLIENT_ID="$(jq -r ".web.client_id" secret/*)"
export GOOGLE_CLIENT_SECRET="$(jq -r ".web.client_secret" secret/*)"
export DB_PATH="$PWD/location-app.sqlite3"

if [ ! -f "$DB_PATH" ]; then
  sqlite3 "$DB_PATH" < "$DIR/db/up.sql"
fi

"$BIN_LOCATION"
