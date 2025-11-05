# Project Structure

AsterDrive is organized with a clean, modular architecture:

```
AsterDrive/
├── src/
│   ├── api/                    # API layer
│   │   ├── auth.rs            # Authentication endpoints (register, login)
│   │   ├── dto.rs             # Data Transfer Objects
│   │   ├── files.rs           # File management endpoints
│   │   ├── health.rs          # Health check endpoint
│   │   └── mod.rs             # Router configuration and OpenAPI setup
│   │
│   ├── auth/                   # Authentication logic
│   │   ├── jwt.rs             # JWT token creation and verification
│   │   ├── middleware.rs      # Authentication middleware
│   │   └── mod.rs             # Module exports
│   │
│   ├── config/                 # Configuration management
│   │   └── mod.rs             # Environment-based config loading
│   │
│   ├── db/                     # Database connection
│   │   └── mod.rs             # PostgreSQL connection setup
│   │
│   ├── models/                 # Database models (SeaORM)
│   │   ├── file.rs            # File entity and relations
│   │   ├── user.rs            # User entity and relations
│   │   └── mod.rs             # Module exports
│   │
│   ├── storage/                # Storage backend implementations
│   │   ├── traits.rs          # StorageBackend trait definition
│   │   ├── local.rs           # Local filesystem implementation
│   │   ├── s3.rs              # S3-compatible implementation
│   │   └── mod.rs             # Backend factory and exports
│   │
│   └── main.rs                 # Application entry point
│
├── migration/                  # Database migrations
│   ├── src/
│   │   ├── lib.rs             # Migration registry
│   │   ├── main.rs            # Migration CLI tool
│   │   ├── m20240101_000001_create_users_table.rs
│   │   └── m20240101_000002_create_files_table.rs
│   └── Cargo.toml             # Migration dependencies
│
├── Cargo.toml                  # Main project dependencies
├── Dockerfile                  # Docker image definition
├── docker-compose.yml          # Docker Compose setup
├── .env.example                # Example environment configuration
├── .gitignore                  # Git ignore rules
├── .dockerignore               # Docker ignore rules
│
├── README.md                   # Main documentation
├── API_EXAMPLES.md             # API usage examples
├── CONTRIBUTING.md             # Contributing guidelines
├── LICENSE                     # MIT License
└── quickstart.sh               # Quick setup script
```

## Module Responsibilities

### API Layer (`src/api/`)
Handles HTTP requests and responses. Contains route handlers, request/response DTOs, and OpenAPI documentation annotations.

### Authentication (`src/auth/`)
Manages JWT tokens and authentication middleware. Handles password hashing and token verification.

### Configuration (`src/config/`)
Loads and validates configuration from environment variables.

### Database (`src/db/`)
Manages database connections using SeaORM.

### Models (`src/models/`)
Defines database entities and their relationships using SeaORM.

### Storage (`src/storage/`)
Provides a pluggable storage system through the `StorageBackend` trait. Currently implements local filesystem and S3-compatible storage.

### Migrations (`migration/`)
Contains database schema migrations managed by SeaORM migration system.

## Key Design Patterns

1. **Trait-based Storage**: The `StorageBackend` trait allows easy addition of new storage providers
2. **Layered Architecture**: Clear separation between API, business logic, and data layers
3. **Dependency Injection**: Application state is passed through Axum's state system
4. **Error Handling**: Custom error types with proper error propagation
5. **Configuration as Code**: Type-safe configuration using Rust structs

## Adding New Components

### New Storage Backend
1. Create a new file in `src/storage/`
2. Implement the `StorageBackend` trait
3. Add factory logic to `src/storage/mod.rs`

### New API Endpoint
1. Add handler function in appropriate `src/api/` file
2. Add OpenAPI documentation attributes
3. Register route in `src/api/mod.rs`

### New Database Model
1. Create entity in `src/models/`
2. Create migration in `migration/src/`
3. Register migration in `migration/src/lib.rs`

## Dependencies

### Core Dependencies
- **axum**: Web framework
- **tokio**: Async runtime
- **sea-orm**: Database ORM
- **serde**: Serialization

### Authentication
- **jsonwebtoken**: JWT token handling
- **bcrypt**: Password hashing

### Storage
- **aws-sdk-s3**: S3 storage support
- **aws-config**: AWS configuration

### Documentation
- **utoipa**: OpenAPI spec generation
- **utoipa-swagger-ui**: Swagger UI integration

## File Counts
- Rust source files: 18
- Migration files: 4
- Documentation files: 5
- Configuration files: 7
