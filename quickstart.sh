#!/bin/bash

# AsterDrive Quick Start Script
# This script helps you quickly set up and run AsterDrive

set -e

echo "🚀 AsterDrive Quick Start"
echo "========================="
echo ""

# Check for required tools
command -v cargo >/dev/null 2>&1 || { echo "❌ Rust/Cargo is required but not installed. Visit https://rustup.rs"; exit 1; }
command -v psql >/dev/null 2>&1 || { echo "❌ PostgreSQL is required but not installed."; exit 1; }

# Database setup
echo "📦 Setting up database..."
DB_NAME="asterdrive"
DB_USER="asterdrive"
DB_PASS="password"

# Check if database exists
if psql -lqt | cut -d \| -f 1 | grep -qw $DB_NAME; then
    echo "✅ Database '$DB_NAME' already exists"
else
    echo "Creating database '$DB_NAME'..."
    createdb $DB_NAME || {
        echo "⚠️  Failed to create database. Trying with psql..."
        psql -U postgres -c "CREATE DATABASE $DB_NAME;"
    }
    echo "✅ Database created"
fi

# Create .env file if it doesn't exist
if [ ! -f .env ]; then
    echo ""
    echo "📝 Creating .env file..."
    cp .env.example .env
    
    # Generate a random JWT secret
    JWT_SECRET=$(openssl rand -base64 32 2>/dev/null || cat /dev/urandom | tr -dc 'a-zA-Z0-9' | fold -w 32 | head -n 1)
    
    # Update .env with generated secret
    if [[ "$OSTYPE" == "darwin"* ]]; then
        sed -i '' "s|JWT__SECRET=.*|JWT__SECRET=$JWT_SECRET|" .env
        sed -i '' "s|DATABASE__URL=.*|DATABASE__URL=postgresql://$DB_USER:$DB_PASS@localhost/$DB_NAME|" .env
    else
        sed -i "s|JWT__SECRET=.*|JWT__SECRET=$JWT_SECRET|" .env
        sed -i "s|DATABASE__URL=.*|DATABASE__URL=postgresql://$DB_USER:$DB_PASS@localhost/$DB_NAME|" .env
    fi
    
    echo "✅ .env file created"
else
    echo "✅ .env file already exists"
fi

echo ""
echo "🔨 Building AsterDrive..."
cargo build --release

echo ""
echo "🎉 Setup complete!"
echo ""
echo "To start AsterDrive, run:"
echo "  cargo run --release"
echo ""
echo "Or use the pre-built binary:"
echo "  ./target/release/asterdrive"
echo ""
echo "The server will be available at:"
echo "  http://localhost:3000"
echo ""
echo "API Documentation:"
echo "  http://localhost:3000/swagger-ui"
echo ""
echo "Happy coding! 🎈"
