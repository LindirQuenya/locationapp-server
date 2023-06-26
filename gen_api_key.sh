#!/usr/bin/env bash
# Database filename. Configure to taste.
DB_PATH="$PWD/location-app.sqlite3"

# Prompt the user for the key lifetime.
declare -i KEY_LIFETIME
echo 'How many days should this key live for?'
read KEY_LIFETIME
if [ $KEY_LIFETIME -le 0 ]; then
  echo "Error: key lifetime must be positive."
  exit 1
fi

# Prompt the user for the associated name.
echo 'Who is this key for?'
read KEY_USERNAME
# Escape it, hopefully this'll deal with accidental SQLi things.
# This is not intended to actually withstand an attack. It's just
# to keep this from breaking if the username has a quote in it.
ESCAPED_KEY_USERNAME="$(echo "$KEY_USERNAME" | sed "s/'/''/g")"

# Generate a 1024-bit random number and base64 it.
APIKEY="$(dd if=/dev/urandom bs=4 count=16 status=none | base64 -w0)"
GENERATED=$(date +%s)
EXPIRY=$((GENERATED + KEY_LIFETIME * 24 * 60 * 60))

# Execute the insertion.
sqlite3 "$DB_PATH" "BEGIN;INSERT INTO api_keys(username, key_base64, issued, expiration) VALUES ('$ESCAPED_KEY_USERNAME', '$APIKEY', $GENERATED, $EXPIRY);COMMIT;"
echo "Your key is: '$APIKEY'"
