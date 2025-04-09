This project is a high-quality, professional example of a Rust web API using Axum, interacting with AWS services. It demonstrates file uploads, database interactions, and a structured application layout following SOLID principles.

## Project Structure

The code is organized into several modules within the `src/` directory to separate concerns:

```
.
├── .env             # Local environment variables (you create this)
├── .env.example     # Example environment variables
├── .localstack/     # Stores LocalStack data if docker-compose volume is used
├── Cargo.toml       # Rust project manifest (dependencies)
├── docker-compose.yml # Defines the LocalStack service for Docker
└── src/             # Source code directory
    ├── main.rs      # Main entry point: orchestrates setup & starts server
    ├── config.rs    # Loads application configuration (e.g., bucket name)
    ├── errors.rs    # Defines custom error types for different layers
    ├── domain.rs    # Defines core logic interfaces (traits) like `MemeRepository`
    ├── repositories.rs # Implements `MemeRepository` using DynamoDB
    ├── storage.rs   # Implements `FileStorage` using S3
    ├── handlers.rs  # Contains the Axum functions that handle specific API requests
    ├── routes.rs    # Defines the API routes and maps them to handlers
    ├── startup.rs   # Handles initialization of AWS resources (table, bucket)
    ├── models.rs    # Defines the core `Meme` data structure
    └── aws_clients.rs # Creates configured AWS SDK clients (for DynamoDB, S3)
```

This layered structure (main -> routes -> handlers -> domain -> repositories/storage) makes the code easier to understand, test, and modify.

## Explanation of Each Module

* **`main.rs`:** The main entry point and "composition root" of the application. It initializes logging, loads configuration (`config.rs`), creates AWS clients (`aws_clients.rs`), ensures resources are ready (`startup.rs`), instantiates concrete repository/storage implementations (`repositories.rs`, `storage.rs`), builds the shared `AppState` by injecting dependencies (as traits), creates the web router (`routes.rs`), and finally starts the Axum HTTP server.

* **`config.rs`:** Defines a `Config` struct to hold all application settings (like server address, S3 bucket name, AWS region). It includes logic to load these values from environment variables and/or a `.env` file, centralizing configuration management.

* **`errors.rs`:** Defines the different error types used throughout the application, separating them by layer. Includes `AppError` for the web layer (which implements Axum's `IntoResponse` to create HTTP error responses), `RepoError` for database operations, and `StorageError` for file storage operations. Also contains `From` trait implementations to convert errors between layers.

* **`domain.rs`:** Specifies the core business logic contracts using Rust traits. Defines `MemeRepository` (operations for meme metadata like create, get, list, delete) and `FileStorage` (operations for files like upload, download, delete). Handlers interact with these traits, abstracting away the specific storage implementations.

* **`repositories.rs`:** Provides the concrete implementation for the `MemeRepository` trait, specifically using AWS DynamoDB. Contains the `DynamoDbMemeRepository` struct which holds the DynamoDB client and translates the trait methods into specific DynamoDB SDK calls (PutItem, GetItem, Scan, DeleteItem). Maps SDK errors to `RepoError`.

* **`storage.rs`:** Provides the concrete implementation for the `FileStorage` trait, specifically using AWS S3. Contains the `S3FileStorage` struct which holds the S3 client and bucket name. Implements the trait methods (`upload`, `download`, `delete`) by making calls to the S3 SDK (PutObject, GetObject, DeleteObject). Maps SDK errors to `StorageError`.

* **`handlers.rs`:** Contains the functions that directly handle incoming HTTP requests for each defined route. They extract data from the request (path parameters, form data, state), utilize the methods defined in the `MemeRepository` and `FileStorage` traits (via `AppState`), process the results, and map outcomes (success or domain errors) into appropriate HTTP responses or `AppError`s.

* **`routes.rs`:** Defines the web API's routes using `axum::Router`. It maps HTTP methods (GET, POST, DELETE) and URL paths to the corresponding functions in `handlers.rs`. It's also where global middleware (like CORS, request logging, body limits) is applied and the shared `AppState` is attached to the router.

* **`startup.rs`:** Encapsulates logic that needs to run once when the application starts to ensure external resources are ready. Currently includes functions to idempotently create the DynamoDB table (`memes`) and the S3 bucket if they don't already exist in the target environment (LocalStack).

* **`models.rs`:** Defines the core data structures representing application entities. Primarily contains the `Meme` struct, specifying its fields (`meme_id`, `title`, `description`, `image_key`) and deriving necessary standard traits (`Serialize`, `Deserialize`, etc.).

* **`aws_clients.rs`:** Responsible solely for creating configured instances of the AWS SDK clients (`DynamoDbClient`, `S3Client`). Handles setting the AWS region, credentials (dummy for LocalStack), and endpoint URL override needed to connect to LocalStack instead of real AWS.

## API Usage Examples

You can interact with the running API using `curl` or tools like Postman.

*(Note: If using standard Windows Command Prompt, you might need to adjust path separators (`\`) and potentially escape characters differently compared to the Linux/bash examples below. PowerShell is generally more compatible with these examples.)*

**1. Upload a Meme**

* **Endpoint:** `POST /upload_meme`
* **Request Type:** `multipart/form-data`
* **Fields:**
    * `title`: (Text) The title of the meme.
    * `description`: (Text) A description.
    * `image`: (File) The image file itself.
* **Example (`curl`):**
    ```bash
    curl -X POST http://localhost:3000/upload_meme \
      -F "title=Red Panda" \
      -F "description=A red panda" \
      -F "image=./red_panda.jpg"
    ```
* **Successful Response (201 Created):**
    ```json
    {
      "meme_id": "a1b2c3d4-e5f6-7890-1234-567890abcdef", // Unique ID generated by server
      "title": "Red Panda",
      "description": "A red panda",
      "image_key": "a1b2c3d4-e5f6-7890-1234-567890abcdef.jpg" // Filename in S3
    }
    ```

**2. Retrieve a Specific Meme's Metadata**

* **Endpoint:** `GET /meme/{id}`
* **Path Parameter:** Replace `{id}` with the `meme_id` you received when uploading (or from the list below).
* **Example (`curl`):**
    ```bash
    # Replace a1b2c3d4-e5f6-7890-1234-567890abcdef with an actual ID
    curl http://localhost:3000/meme/a1b2c3d4-e5f6-7890-1234-567890abcdef
    ```
* **Successful Response (200 OK):**
    ```json
    {
      "meme_id": "a1b2c3d4-e5f6-7890-1234-567890abcdef",
      "title": "Red Panda",
      "description": "A red panda",
      "image_key": "a1b2c3d4-e5f6-7890-1234-567890abcdef.jpg"
    }
    ```
* **Not Found Response (404 Not Found):**
    ```json
    {
      "error": "Meme not found with ID: a1b2c3d4-0000-0000-0000-567890abcdef"
    }
    ```

**3. List All Memes' Metadata**

* **Endpoint:** `GET /memes`
* **Example (`curl`):**
    ```bash
    curl http://localhost:3000/memes
    ```
* **Successful Response (200 OK):**
    ```json
    [
      {
        "meme_id": "a1b2c3d4-e5f6-7890-1234-567890abcdef",
        "title": "Red Panda",
        "description": "A red panda",
        "image_key": "a1b2c3d4-e5f6-7890-1234-567890abcdef.jpg"
      },
      {
        "meme_id": "b2c3d4e5-f6a7-8901-2345-67890abcdef0",
        "title": "Another Meme",
        "description": "Something funny",
        "image_key": "b2c3d4e5-f6a7-8901-2345-67890abcdef0.png"
      }
      // ... more memes
    ]
    ```

**4. Retrieve a Meme Image**

* **Endpoint:** `GET /images/{key}`
* **Path Parameter:** Replace `{key}` with the `image_key` from a meme's metadata (e.g., `a1b2c3d4-e5f6-7890-1234-567890abcdef.jpg`).
* **How it Works:** This endpoint acts as a proxy. When you request it, the API fetches the image file directly from the S3 storage (LocalStack) and streams the image data back to you in the response with the correct Content-Type.
* **Example (Browser / `<img>` tag):**
    You can use this URL directly in an HTML `<img>` tag:
    ```html
    <img src="http://localhost:3000/images/a1b2c3d4-e5f6-7890-1234-567890abcdef.jpg" alt="My Cat Meme">
    ```
    *(Replace the image key with a real one from an uploaded meme)*
* **Example (`curl`):**
    * Use `curl` with the `-o` flag to save the output to a file.
    ```bash
    # Replace the image key with a real one
    curl http://localhost:3000/images/a1b2c3d4-e5f6-7890-1234-567890abcdef.jpg -o output_image.jpg
    ```
    * This will download the image and save it as `output_image.jpg`.
* **Successful Response (200 OK):** The raw image data with the appropriate `Content-Type` header (e.g., `image/jpeg`).
* **Not Found Response (404 Not Found):**
    ```json
    {
      "error": "Image not found with key: non_existent_key.png"
    }
    ```

**5. Delete a Meme**

* **Endpoint:** `DELETE /meme/{id}`
* **Path Parameter:** Replace `{id}` with the `meme_id` of the meme you want to delete.
* **How it Works:** This request tells the API to delete both the meme's metadata from the database and the associated image file from storage.
* **Example (`curl`):**
    ```bash
    # Replace a1b2c3d4-e5f6-7890-1234-567890abcdef with an actual ID
    curl -X DELETE http://localhost:3000/meme/a1b2c3d4-e5f6-7890-1234-567890abcdef
    ```
* **Successful Response (204 No Content):** No JSON body is returned, just the HTTP status code indicating success.
* **Not Found Response (404 Not Found):** If you try to delete a meme ID that doesn't exist.
    ```json
    {
      "error": "Meme metadata not found with ID: a1b2c3d4-0000-0000-0000-567890abcdef"
    }
    ```