# AsterDrive

A self-hosted cloud storage system inspired by Cloudreve, built with Rust and Axum. AsterDrive provides a modular, high-performance file storage solution with clean architecture and easy extensibility.

## Features

- 🔐 **User Authentication**: JWT-based authentication with secure password hashing
- 👥 **Multi-User Support**: Multiple users with isolated file storage
- 📤 **File Upload/Download**: RESTful API for file operations with resumable upload support
- 🔌 **Pluggable Storage Backends**: Trait-based system supporting Local and S3-compatible storage
- 🗄️ **PostgreSQL Database**: File metadata stored using SeaORM
- 📚 **OpenAPI Documentation**: Auto-generated API documentation with Swagger UI
- 🚀 **High Performance**: Built with Rust and async/await for maximum efficiency
- 🏗️ **Clean Architecture**: Modular design for easy maintenance and extension

## Architecture

AsterDrive follows a clean, modular architecture:

```
src/
├── api/          # API routes and handlers
├── auth/         # Authentication and JWT management
├── config/       # Configuration management
├── db/           # Database connection
├── models/       # SeaORM entity models
└── storage/      # Storage backend implementations
    ├── traits.rs # Storage trait definition
    ├── local.rs  # Local filesystem storage
    └── s3.rs     # S3-compatible storage
```

## Prerequisites

- Rust 1.70 or later
- PostgreSQL 12 or later
- (Optional) S3-compatible object storage for S3 backend

## Quick Start

### 1. Clone the repository

```bash
git clone https://github.com/AptS-1547/AsterDrive.git
cd AsterDrive
```

### 2. Set up PostgreSQL

```bash
# Create database
createdb asterdrive

# Or using psql
psql -U postgres
CREATE DATABASE asterdrive;
```

### 3. Configure environment variables

```bash
cp .env.example .env
# Edit .env with your settings
```

Required environment variables:
- `DATABASE__URL`: PostgreSQL connection string
- `JWT__SECRET`: Secret key for JWT tokens (change in production!)
- `STORAGE__BACKEND`: Storage backend type (`local` or `s3`)

### 4. Build and run

```bash
# Build the project
cargo build --release

# Run the server
cargo run --release
```

The server will start at `http://127.0.0.1:3000` by default.

## API Documentation

Once the server is running, visit:
- **Swagger UI**: http://localhost:3000/swagger-ui
- **OpenAPI JSON**: http://localhost:3000/api-docs/openapi.json

## API Endpoints

### Authentication

- `POST /api/auth/register` - Register a new user
- `POST /api/auth/login` - Login and get JWT token

### Files (Authenticated)

- `POST /api/files/upload` - Upload a file
- `GET /api/files` - List user's files
- `GET /api/files/{id}` - Download a file
- `DELETE /api/files/{id}` - Delete a file

## Configuration

### Local Storage

```env
STORAGE__BACKEND=local
STORAGE__LOCAL_PATH=./data/uploads
```

### S3-Compatible Storage

```env
STORAGE__BACKEND=s3
STORAGE__S3_BUCKET=my-bucket
STORAGE__S3_REGION=us-east-1
STORAGE__S3_ENDPOINT=https://s3.amazonaws.com
```

For MinIO or other S3-compatible services, set the `STORAGE__S3_ENDPOINT` to your service URL.

## Development

### Running in development mode

```bash
# With debug logging
RUST_LOG=debug cargo run
```

### Running tests

```bash
cargo test
```

### Database Migrations

Migrations are automatically applied on startup. To manage migrations manually:

```bash
# Create a new migration
cargo install sea-orm-cli
sea-orm-cli migrate generate <migration_name>

# Apply migrations
sea-orm-cli migrate up

# Rollback migrations
sea-orm-cli migrate down
```

## Storage Backend Extension

To add a new storage backend:

1. Create a new module in `src/storage/`
2. Implement the `StorageBackend` trait
3. Update `src/storage/mod.rs` to register the new backend

Example:

```rust
use async_trait::async_trait;
use super::traits::{StorageBackend, StorageResult};

pub struct MyCustomStorage {
    // Your storage fields
}

#[async_trait]
impl StorageBackend for MyCustomStorage {
    async fn store(&self, path: &str, data: Bytes) -> StorageResult<String> {
        // Implementation
    }
    
    // Implement other required methods...
}
```

## Example Usage

### Register a user

```bash
curl -X POST http://localhost:3000/api/auth/register \
  -H "Content-Type: application/json" \
  -d '{
    "username": "john",
    "email": "john@example.com",
    "password": "secretpassword"
  }'
```

### Login

```bash
curl -X POST http://localhost:3000/api/auth/login \
  -H "Content-Type: application/json" \
  -d '{
    "username": "john",
    "password": "secretpassword"
  }'
```

### Upload a file

```bash
curl -X POST http://localhost:3000/api/files/upload \
  -H "Authorization: Bearer YOUR_JWT_TOKEN" \
  -F "file=@/path/to/file.pdf"
```

### List files

```bash
curl http://localhost:3000/api/files \
  -H "Authorization: Bearer YOUR_JWT_TOKEN"
```

### Download a file

```bash
curl http://localhost:3000/api/files/1 \
  -H "Authorization: Bearer YOUR_JWT_TOKEN" \
  -o downloaded_file.pdf
```

## Security Considerations

- **Always change the JWT secret** in production environments
- Use HTTPS in production
- Configure CORS appropriately for your use case
- Implement rate limiting for production deployments
- Consider adding file type validation and size limits
- Use strong database passwords

## Performance Tips

- For S3 storage, use a CDN for file delivery
- Enable compression for API responses
- Consider implementing caching for file metadata
- Use connection pooling for database connections
- Monitor and optimize database queries

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.

## License

This project is open source and available under the MIT License.

## Acknowledgments

Inspired by [Cloudreve](https://github.com/cloudreve/Cloudreve), this project aims to provide a Rust-native alternative with modern architecture and extensibility.
