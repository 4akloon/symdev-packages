//! `serve`: the install test's fake bucket, a static file server on 127.0.0.1.

mod static_server;

pub use static_server::StaticServer;

#[cfg(test)]
mod tests;
