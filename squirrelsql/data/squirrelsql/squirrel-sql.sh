#!/bin/sh

SQUIRREL_SQL_HOME=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
exec "$SNAP/usr/lib/jvm/default-java/bin/java" \
  -jar "$SQUIRREL_SQL_HOME/squirrel-sql.jar" \
  --log-config-file "$SQUIRREL_SQL_HOME/log4j.properties" \
  --squirrel-home "$SQUIRREL_SQL_HOME" \
  "$@"
