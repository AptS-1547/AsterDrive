# API Testing Examples

This document provides curl examples for testing all API endpoints.

## Setup

First, start the server (either locally or with Docker):

```bash
# Local
cargo run

# Or with Docker
docker-compose up
```

The server will be available at `http://localhost:3000`.

## 1. Health Check

```bash
curl http://localhost:3000/health
```

Expected response:
```json
{
  "status": "healthy",
  "service": "AsterDrive",
  "version": "0.1.0"
}
```

## 2. User Registration

```bash
curl -X POST http://localhost:3000/api/auth/register \
  -H "Content-Type: application/json" \
  -d '{
    "username": "alice",
    "email": "alice@example.com",
    "password": "secure_password_123"
  }'
```

Expected response:
```json
{
  "token": "eyJ0eXAiOiJKV1QiLCJhbGc...",
  "user": {
    "id": 1,
    "username": "alice",
    "email": "alice@example.com",
    "is_active": true
  }
}
```

## 3. User Login

```bash
curl -X POST http://localhost:3000/api/auth/login \
  -H "Content-Type: application/json" \
  -d '{
    "username": "alice",
    "password": "secure_password_123"
  }'
```

Expected response:
```json
{
  "token": "eyJ0eXAiOiJKV1QiLCJhbGc...",
  "user": {
    "id": 1,
    "username": "alice",
    "email": "alice@example.com",
    "is_active": true
  }
}
```

## 4. Upload File

Save the JWT token from login/register and use it for authenticated requests:

```bash
TOKEN="your_jwt_token_here"

# Upload a text file
echo "Hello, AsterDrive!" > test.txt
curl -X POST http://localhost:3000/api/files/upload \
  -H "Authorization: Bearer $TOKEN" \
  -F "file=@test.txt"
```

Expected response:
```json
{
  "id": 1,
  "filename": "550e8400-e29b-41d4-a716-446655440000",
  "size": 19,
  "mime_type": "text/plain"
}
```

## 5. List Files

```bash
curl http://localhost:3000/api/files \
  -H "Authorization: Bearer $TOKEN"
```

Expected response:
```json
{
  "files": [
    {
      "id": 1,
      "filename": "550e8400-e29b-41d4-a716-446655440000",
      "original_filename": "test.txt",
      "size": 19,
      "mime_type": "text/plain",
      "is_public": false,
      "created_at": "2024-01-01T12:00:00Z"
    }
  ],
  "total": 1
}
```

## 6. Download File

```bash
FILE_ID=1
curl http://localhost:3000/api/files/$FILE_ID \
  -H "Authorization: Bearer $TOKEN" \
  -o downloaded_file.txt
```

## 7. Delete File

```bash
FILE_ID=1
curl -X DELETE http://localhost:3000/api/files/$FILE_ID \
  -H "Authorization: Bearer $TOKEN"
```

Expected response: HTTP 204 No Content

## Complete Test Script

Here's a complete bash script to test all endpoints:

```bash
#!/bin/bash

BASE_URL="http://localhost:3000"

echo "1. Testing health check..."
curl -s $BASE_URL/health | jq .

echo -e "\n2. Registering user..."
REGISTER_RESPONSE=$(curl -s -X POST $BASE_URL/api/auth/register \
  -H "Content-Type: application/json" \
  -d '{
    "username": "testuser",
    "email": "test@example.com",
    "password": "test123"
  }')

TOKEN=$(echo $REGISTER_RESPONSE | jq -r .token)
echo "Token: $TOKEN"

echo -e "\n3. Creating test file..."
echo "This is a test file" > /tmp/test_upload.txt

echo -e "\n4. Uploading file..."
UPLOAD_RESPONSE=$(curl -s -X POST $BASE_URL/api/files/upload \
  -H "Authorization: Bearer $TOKEN" \
  -F "file=@/tmp/test_upload.txt")

FILE_ID=$(echo $UPLOAD_RESPONSE | jq -r .id)
echo "Uploaded file ID: $FILE_ID"

echo -e "\n5. Listing files..."
curl -s $BASE_URL/api/files \
  -H "Authorization: Bearer $TOKEN" | jq .

echo -e "\n6. Downloading file..."
curl -s $BASE_URL/api/files/$FILE_ID \
  -H "Authorization: Bearer $TOKEN" \
  -o /tmp/downloaded_file.txt

echo -e "\nDownloaded content:"
cat /tmp/downloaded_file.txt

echo -e "\n\n7. Deleting file..."
curl -s -X DELETE $BASE_URL/api/files/$FILE_ID \
  -H "Authorization: Bearer $TOKEN"

echo -e "\n\nTest complete!"
```

Save this as `test_api.sh`, make it executable with `chmod +x test_api.sh`, and run it.

## API Documentation

For interactive API documentation, visit:
- **Swagger UI**: http://localhost:3000/swagger-ui

## Error Responses

All error responses follow this format:

```json
{
  "error": "Error message description"
}
```

Common HTTP status codes:
- `200 OK` - Success
- `201 Created` - Resource created
- `204 No Content` - Success with no content
- `400 Bad Request` - Invalid request
- `401 Unauthorized` - Authentication required or failed
- `403 Forbidden` - Insufficient permissions
- `404 Not Found` - Resource not found
- `409 Conflict` - Resource already exists
- `500 Internal Server Error` - Server error
