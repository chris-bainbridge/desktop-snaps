#!/bin/sh
set -eu

if [ "$#" -ne 0 ]; then
  echo "Usage: $0" >&2
  exit 2
fi

OUTPUT=$(snap run --shell squirrelsql -c '
  set -eu
  SQUIRREL_HOME="$SNAP/squirrelsql"
  USER_DIR=$(mktemp -d "$SNAP_USER_DATA/squirrel-sqlite-test.XXXXXX")
  trap '\''rm -rf "$USER_DIR"'\'' EXIT HUP INT TERM
  "$JAVA_HOME/bin/java" -Djava.awt.headless=true \
    -cp "$SQUIRREL_HOME/squirrel-sql.jar:$SQUIRREL_HOME/lib/*" \
    -Dsquirrel.home="$SQUIRREL_HOME" \
    -Dsquirrel.userdir="$USER_DIR" \
    net.sourceforge.squirrel_sql.client.cli.SquirrelBatch \
    -userdir "$USER_DIR" \
    -driver org.sqlite.JDBC \
    -drivercp "$SQUIRREL_HOME/lib/sqlite-jdbc.jar" \
    -url jdbc:sqlite::memory: \
    -user "" \
    -sql "select 42 as smoke_test"
' 2>&1) || {
    printf '%s\n' "$OUTPUT" >&2
    exit 1
  }

printf '%s\n' "$OUTPUT"
case "$OUTPUT" in
  *42*) ;;
  *) echo "SQuirreL SQLite query did not return 42" >&2; exit 1 ;;
esac
