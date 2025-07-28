# Error Handling

## 1. match

```rust
let mut conn = get_connection("file_index.db")
    .map(|conn| {
        println!("Database connection established.");
        conn
    })
    .map_err(|e| {
        eprintln!("Failed to connect to database: {}", e);
        io::Error::new(io::ErrorKind::Other, e.to_string())
    })?;
```

This example uses `.map()` to handle the success case (printing a message and returning the connection) and `.map_err()` to handle the error case (printing an error and converting it to an `io::Error`). The `?` operator propagates the error if one occurs.

## 2. error variable definition

You can define an error variable to reuse or inspect the error before handling it:

```rust
let conn_result = get_connection("file_index.db");
if let Err(e) = &conn_result {
    eprintln!("Failed to connect to database: {}", e);
}
let mut conn = conn_result
    .map_err(|e| io::Error::new(io::ErrorKind::Other, e.to_string()))?;
```

This approach allows you to log, inspect, or branch on the error before converting or propagating it.

## 3. map_err

The `.map_err()` method is used to convert one error type into another. This is especially useful when your function must return a specific error type, such as `std::io::Error`:

```rust
let mut conn = get_connection("file_index.db")
    .map_err(|e| {
        eprintln!("Failed to connect to database: {}", e);
        io::Error::new(io::ErrorKind::Other, e.to_string())
    })?;
```

Here, any error from `get_connection` is logged and converted into an `io::Error`, which matches the return type of the function.

## 4. The `?` Operator

The `?` operator is used to propagate errors. If the result is `Ok`, it unwraps the value; if it is `Err`, it returns early from the function with that error:

```rust
let mut conn = get_connection("file_index.db")
    .map_err(|e| io::Error::new(io::ErrorKind::Other, e.to_string()))?;
```

This keeps error handling concise and idiomatic.

## 5. Summary

- Use `.map()` for handling success values.
- Use `.map_err()` to convert error types and log errors.
- Use the `?` operator to propagate errors.
- You can define error variables for more complex error handling logic.

This approach leads to clear, robust, and idiomatic error handling in Rust.