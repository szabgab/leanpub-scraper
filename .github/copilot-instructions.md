# Copilot Instructions

This file provides custom instructions for GitHub Copilot in this repository.

## Project
- Use async/await for all asynchronous Rust code.
- Prefer using the `playwright` crate for HTTP requests.
- Follow Rust 2024 edition best practices.
- Add comments to all public functions.

## Project description

This is a command-line application accessing the https://leanpub.com/ web site and providing a way for authors to manage multiple books.

* The user credentials are stored in the .env file which is ignored by git.
- Login to the website throught this form: https://leanpub.com/login save the cookie returned by the server.
- Use the saved cookie for the following requests
- List all the published books authored by the logged in user via this page: https://leanpub.com/author_dashboard/books/published
- List all the unpublished books authored by the logged in user via this page:  https://leanpub.com/author_dashboard/books/unpublished
- List the categories of all the books authored by the logged in user. For each book use this URL where SLUG is the URL of each book https://leanpub.com/SLUG/book_categories
