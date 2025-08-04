#!/usr/bin/env bash
# Database filename. Configure to taste.
DB_PATH="$PWD/location-app.sqlite3"

# Prompt the user for the key lifetime.
declare -i KEY_LIFETIME
echo 'How many days should this user have access?'
read KEY_LIFETIME
if [[ $KEY_LIFETIME -le 0 ]]; then
  echo "Error: key lifetime must be positive."
  exit 1
fi

# Prompt the user for the associated name.
echo 'Whom is this access for?'
read USERNAME
# Escape it, hopefully this'll deal with accidental SQLi things.
# This is not intended to actually withstand an attack. It's just
# to keep this from breaking if the username has a quote in it.
ESCAPED_USERNAME="$(echo "$USERNAME" | sed "s/'/''/g")"

# Ask the user for the email to authorize.
echo "What is the user's email address?"
read USER_EMAIL

# Lifted from https://www.regular-expressions.info/email.html
if [[ ! "${USER_EMAIL^^}" =~ ^[A-Z0-9._%+-]+@[A-Z0-9.-]+\.[A-Z]{2,63}$ ]]; then
  echo "Error: not an email."
fi

GENERATED=$(date +%s)
EXPIRY=$((GENERATED + KEY_LIFETIME * 24 * 60 * 60))

# Execute the insertion.
sqlite3 "$DB_PATH" "BEGIN;INSERT INTO web_users(username, email, issued, expiration) VALUES ('$ESCAPED_USERNAME', '$USER_EMAIL', $GENERATED, $EXPIRY);COMMIT;"
