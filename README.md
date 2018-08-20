# Web example: Login with warp, ructe, and diesel

This application is intended as an example of a web service handling a login
session.
It uses the *warp* web framework, the *ructe* template engine and the
*diesel* database layer.

[![Build Status](https://travis-ci.org/kaj/warp-diesel-ructe-sample.svg?branch=master)](https://travis-ci.org/kaj/warp-diesel-ructe-sample)

A `Session` object is created for each request (except for static resources),
containing a handle to a database connection pool and an `Option<User>` that
is set if the user is logged in.

The authentication are done with bcrypt verification of hashed passwords (the
hashes are stored in the database, passwords are never stored or logged in
plain text).

When authenticated, the user gets a cookie (httponly, strict samesite)
containing a session key, which is used for authentication through the
remainder of the session.

## Things that could use improvement:

* Mainly, the routing provieded by wrap is very nice.
  But it would be nice to be able to define routers and subrouters in a more
  tree-like way.
  Perhaps it is possible, and I just havn't found out how yet?

* I have probably missed something in how errors are supposed to be handled
  in warp.
  It feels like I am wrapping `Result`s in `Result`s, and I use more
  `.map_err(...)` than I like.

* Database is not really handled asyncronously yet, so database accesses
  blocks the worker.
  See https://github.com/diesel-rs/diesel/issues/399 for information.

## Things that remains to be done:

* Session keys should have a limited age.
  Maybe doing a request after half that time should generate a new session
  key?

* The code that handles the authentication and sessions should be
  externalized to a separate crate, but the session data should remain
  application-specific.

* CSRF protection is not yet implemented.

This project was partially inspired by
https://github.com/rust-lang-nursery/wg-net/issues/44 ;
go there for more example projects.

Issue reports and pull requests and welcomed.
