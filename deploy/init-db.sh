#!/bin/bash
# Creates the second (data) database alongside the default config database.
# The hub's migrations enable the timescaledb extension inside vantage_data.
set -e

psql -v ON_ERROR_STOP=1 --username "$POSTGRES_USER" --dbname "$POSTGRES_DB" <<-EOSQL
    CREATE DATABASE vantage_data;
EOSQL
