# Contributing to AsterDrive

Thank you for your interest in contributing to AsterDrive! This document provides guidelines and instructions for contributing.

## Development Setup

### Prerequisites

- Rust 1.70 or later
- PostgreSQL 12 or later
- Git

### Local Development

1. **Clone the repository**

```bash
git clone https://github.com/AptS-1547/AsterDrive.git
cd AsterDrive
```

2. **Set up PostgreSQL**

```bash
# Create database
createdb asterdrive

# Or using psql
psql -U postgres
CREATE DATABASE asterdrive;
```

3. **Configure environment**

```bash
cp .env.example .env
# Edit .env with your configuration
```

4. **Run migrations**

Migrations are automatically run when you start the server, or you can run them manually:

```bash
cd migration
cargo run --bin migrate
```

5. **Run the server**

```bash
cargo run
```

The server will be available at `http://localhost:3000`.

## Project Structure

```
AsterDrive/
├── src/
│   ├── api/          # API routes and handlers
│   │   ├── auth.rs   # Authentication endpoints
│   │   ├── files.rs  # File management endpoints
│   │   ├── health.rs # Health check endpoint
│   │   ├── dto.rs    # Data transfer objects
│   │   └── mod.rs    # Router configuration
│   ├── auth/         # Authentication logic
│   │   ├── jwt.rs    # JWT token management
│   │   ├── middleware.rs # Auth middleware
│   │   └── mod.rs
│   ├── config/       # Configuration management
│   ├── db/           # Database connection
│   ├── models/       # SeaORM entities
│   │   ├── user.rs   # User model
│   │   ├── file.rs   # File model
│   │   └── mod.rs
│   ├── storage/      # Storage backends
│   │   ├── traits.rs # Storage trait definition
│   │   ├── local.rs  # Local filesystem storage
│   │   ├── s3.rs     # S3-compatible storage
│   │   └── mod.rs
│   └── main.rs       # Application entry point
├── migration/        # Database migrations
└── Cargo.toml
```

## Adding New Features

### Adding a New Storage Backend

1. Create a new file in `src/storage/` (e.g., `azure.rs`)
2. Implement the `StorageBackend` trait
3. Update `src/storage/mod.rs` to include your backend

Example:

```rust
use super::traits::{StorageBackend, StorageResult};
use async_trait::async_trait;
use bytes::Bytes;

pub struct AzureStorage {
    // Your fields
}

#[async_trait]
impl StorageBackend for AzureStorage {
    async fn store(&self, path: &str, data: Bytes) -> StorageResult<String> {
        // Implementation
    }
    
    // Implement other required methods...
}
```

### Adding a New API Endpoint

1. Add your handler function in the appropriate file in `src/api/`
2. Add the route to `src/api/mod.rs`
3. Update the OpenAPI documentation attributes

Example:

```rust
/// My new endpoint
#[utoipa::path(
    get,
    path = "/api/my-endpoint",
    responses(
        (status = 200, description = "Success", body = MyResponse)
    ),
    tag = "my-category"
)]
pub async fn my_endpoint(
    State(state): State<AppState>,
) -> Result<impl IntoResponse, (StatusCode, Json<ErrorResponse>)> {
    // Implementation
}
```

### Adding Database Models

1. Create entity in `src/models/`
2. Create a migration in `migration/src/`
3. Update the migrator in `migration/src/lib.rs`

## Code Style

### Rust Guidelines

- Follow the [Rust Style Guide](https://doc.rust-lang.org/1.0.0/style/)
- Run `cargo fmt` before committing
- Run `cargo clippy` to catch common mistakes
- Keep functions small and focused
- Write descriptive variable names
- Add documentation comments for public APIs

### Error Handling

- Use the `Result` type for operations that can fail
- Create custom error types using `thiserror`
- Provide meaningful error messages
- Don't use `unwrap()` or `expect()` in production code

### Testing

- Write unit tests for business logic
- Write integration tests for API endpoints
- Aim for high test coverage
- Use descriptive test names

Example:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_storage_upload_and_retrieve() {
        // Test implementation
    }
}
```

## Submitting Changes

### Pull Request Process

1. **Fork the repository**

2. **Create a feature branch**

```bash
git checkout -b feature/my-new-feature
```

3. **Make your changes**
   - Write clear, concise commit messages
   - Keep commits focused and atomic
   - Add tests for new functionality
   - Update documentation as needed

4. **Run checks**

```bash
# Format code
cargo fmt

# Run linter
cargo clippy

# Run tests
cargo test

# Build project
cargo build
```

5. **Push to your fork**

```bash
git push origin feature/my-new-feature
```

6. **Open a pull request**
   - Provide a clear description of your changes
   - Reference any related issues
   - Include screenshots for UI changes
   - Ensure CI passes

### Commit Message Guidelines

Follow the [Conventional Commits](https://www.conventionalcommits.org/) specification:

```
<type>: <description>

[optional body]

[optional footer]
```

Types:
- `feat`: New feature
- `fix`: Bug fix
- `docs`: Documentation changes
- `style`: Code style changes (formatting, etc.)
- `refactor`: Code refactoring
- `test`: Adding or updating tests
- `chore`: Maintenance tasks

Examples:

```
feat: add Azure Blob Storage backend

Implements the StorageBackend trait for Azure Blob Storage,
allowing users to use Azure as a storage provider.

Closes #123
```

```
fix: prevent race condition in file upload

Add mutex lock to prevent concurrent uploads from
corrupting file metadata.
```

## Development Tips

### Using cargo-watch for auto-reload

```bash
cargo install cargo-watch
cargo watch -x run
```

### Testing with Docker

```bash
docker-compose up --build
```

### Database Management

```bash
# Connect to database
psql postgresql://asterdrive:password@localhost/asterdrive

# List tables
\dt

# Describe table
\d users
```

### Debugging

Set the `RUST_LOG` environment variable for detailed logging:

```bash
RUST_LOG=debug cargo run
```

## Getting Help

- Open an issue for bug reports or feature requests
- Join discussions in pull requests
- Check existing issues before creating new ones

## Code of Conduct

- Be respectful and inclusive
- Welcome newcomers
- Focus on constructive feedback
- Assume good intentions

## License

By contributing, you agree that your contributions will be licensed under the same license as the project (MIT License).

Thank you for contributing to AsterDrive! 🚀
