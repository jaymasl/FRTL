#!/bin/bash

# Get the directory where the script is located
SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"

# Database configuration
DB_NAME="frtl"
BACKUP_FILE="${SCRIPT_DIR}/frtl_backup_$(date +"%Y%m%d_%H%M%S").sql"

# Create backup
echo "Creating database backup..."
sudo -u postgres pg_dump "${DB_NAME}" > "${BACKUP_FILE}"

if [ $? -eq 0 ]; then
    echo "Backup successfully created at: ${BACKUP_FILE}"
    echo ""
    echo "To restore this backup later, run:"
    echo "sudo -u postgres psql ${DB_NAME} < ${BACKUP_FILE}"
else
    echo "Error: Backup failed"
    exit 1
fi