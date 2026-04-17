## Documentation

- Use inline comments to explain "why," not just "what".
- Don't add narrative comments in function bodies. Only add a comment if what you're doing is non-obvious or special in some way, or if something needs a deeper "why" explanation.
- Module-level documentation should explain purpose and responsibilities.
- **Always** use periods at the end of code comments.
- **Never** use title case in headings and titles. Always use sentence case.
- Always use the Oxford comma.
- Don't omit articles ("a", "an", "the"). Write "the file has a newer version" not "file has newer version".
- Code comments are English sentences that start with an uppercase letter and end with a period. Code identifiers or snippets that appear in code comments must be enclused in backticks quotes.

## Rust edition and formatting

- Use Rust 2024 edition.
- Format with `cargo fmt` after each code change.

## Test organization

- Unit tests in the same file as the code they test.
