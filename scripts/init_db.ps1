Param(
    # Allow to skip Docker if a dockerized Postgres database is already running
    [bool]$SKIP_DOCKER=$False,
    # Check if a custom user has been set, otherwise default to 'postgres'
    [string]$DB_USER = "postgres",
    # Check if a custom password has been set, otherwise default to 'password'
    [string]$DB_PASSWORD = "password",
    # Check if a custom database name has been set, otherwise default to 'newsletter'
    [string]$DB_NAME = "newsletter",
    # Check if a custom port has been set, otherwise default to '5432'
    [int]$DB_PORT = 5432,
    # Check if a custom host has been set, otherwise default to 'localhost'
    [string]$DB_HOST = "localhost"
)

if ($null -eq (Get-Command "psql" -ErrorAction SilentlyContinue)) {
    Write-Error "Error: psql is not installed"
    Exit 1
}

if ($null -eq (Get-Command "sqlx" -ErrorAction SilentlyContinue)) {
    Write-Error "Error: sqlx is not installed"
    Write-Error "Use:"
    Write-Error "   cargo install --version='~0.6' sqlx-cli --no-default-features --features rustls,postgres"
    Write-Error "to install it."
    Exit 1
}

if (-Not $SKIP_DOCKER) {
    Write-Host "Starting postgres docker container..."

    $RUNNING_POSTGRES_CONTAINER=(docker ps --filter 'name=postgres' --format '{{.ID}}')
    if (-Not ($null -eq $RUNNING_POSTGRES_CONTAINER)) {
        Write-Host "There is a Postgres container already running, kill it with"
        Write-Host "    docker kill $RUNNING_POSTGRES_CONTAINER"
        Exit 1
    }

    docker run `
        -e POSTGRES_USER=$DB_USER `
        -e POSTGRES_PASSWORD=$DB_PASSWORD `
        -e POSTGRES_DB=$DB_NAME `
        -p "${DB_PORT}:5432" `
        -d `
        --name "postgres_$(Get-Date -Format "yyyy-MM-dd_HH-mm-ss")" `
        postgres -N 1000
}

$Env:DATABASE_URL="postgres://${DB_USER}:${DB_PASSWORD}@${DB_HOST}:${DB_PORT}/${DB_NAME}"

pg_isready -h $DB_HOST -U $DB_USER -p $DB_PORT -d 'postgres' -q
$IS_DB_READY=$LASTEXITCODE
while (-Not (0 -eq $IS_DB_READY)) {
    Write-Host "Postgress is still unavailable - sleeping"
    Start-Sleep -Seconds 1
    pg_isready -h $DB_HOST -U $DB_USER -p $DB_PORT -d 'postgres' -q
    $IS_DB_READY=$LASTEXITCODE
}
Write-Host "Postgres is up and running on port $DB_PORT"

Write-Host "Running migrations"
sqlx database create
sqlx migrate run
Write-Host "Postgres has been migrated"