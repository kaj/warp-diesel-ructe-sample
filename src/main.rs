//! An example web service using ructe with the warp framework.
#![deny(warnings)]
#[macro_use]
extern crate diesel;

mod error;
mod models;
mod schema;
mod session;

use diesel::insert_into;
use diesel::prelude::*;
use dotenv::dotenv;
use error::Error;
use log::info;
use serde::Deserialize;
use session::{create_session_filter, Session};
use std::env;
use std::io::{self, Write};
use std::time::{Duration, SystemTime};
use templates::statics::StaticFile;
use templates::RenderRucte;
use warp::http::{header, response::Builder, StatusCode};
use warp::{reply::Response, Filter, Rejection, Reply};

/// Main program: Set up routes and start server.
#[tokio::main]
async fn main() {
    dotenv().ok();
    env_logger::init();

    // Get a filter that adds a session to each request.
    let pgsess = create_session_filter(
        &env::var("DATABASE_URL").expect("DATABASE_URL must be set"),
    );
    let s = move || pgsess.clone();

    use warp::{body, get, path, path::end, post};
    let static_routes = get()
        .and(path("static"))
        .and(path::param())
        .then(static_file);
    let routes = warp::any()
        .and(static_routes)
        .or(path("login").and(end()).and(
            get()
                .and(s())
                .then(login_form)
                .or(post().and(s()).and(body::form()).then(do_login))
                .unify(),
        ))
        .unify()
        .or(path("logout")
            .and(end())
            .and(post())
            .and(s())
            .then(do_logout))
        .unify()
        .or(path("signup").and(end()).and(
            get()
                .and(s())
                .then(signup_form)
                .or(post().and(s()).and(body::form()).then(do_signup))
                .unify(),
        ))
        .unify()
        .or(s().and(end()).and(get()).then(home_page))
        .unify()
        .map(|result: Result| match result {
            Ok(reply) => reply,
            Err(err) => err.into_response(),
        })
        .recover(customize_error);
    warp::serve(routes).run(([127, 0, 0, 1], 3030)).await;
}

type Result<T = Response, E = Error> = std::result::Result<T, E>;

/// Render a login form.
async fn login_form(session: Session) -> Result {
    Ok(Builder::new().html(|o| templates::login(o, &session, None, None))?)
}

/// Verify a login attempt.
///
/// If the credentials in the LoginForm are correct, redirect to the
/// home page.
/// Otherwise, show the login form again, but with a message.
async fn do_login(mut session: Session, form: LoginForm) -> Result {
    if let Some(cookie) = session.authenticate(&form.user, &form.password) {
        Ok(Builder::new()
            .status(StatusCode::FOUND)
            .header(header::LOCATION, "/")
            .header(
                header::SET_COOKIE,
                format!("EXAUTH={}; SameSite=Strict; HttpOpnly", cookie),
            )
            .html(|o| writeln!(o))?)
    } else {
        Ok(Builder::new().html(|o| {
            templates::login(o, &session, None, Some("Authentication failed"))
        })?)
    }
}

async fn do_logout(mut session: Session) -> Result {
    session.clear();
    Ok(Builder::new()
        .status(StatusCode::FOUND)
        .header(header::LOCATION, "/")
        .header(
            header::SET_COOKIE,
            "EXAUTH=; Max-Age=0; SameSite=Strict; HttpOpnly",
        )
        .html(|o| writeln!(o))?)
}

/// The data submitted by the login form.
/// This does not derive Debug or Serialize, as the password is plain text.
#[derive(Deserialize)]
struct LoginForm {
    user: String,
    password: String,
}

/// Render a signup form.
async fn signup_form(session: Session) -> Result {
    Ok(Builder::new().html(|o| templates::signup(o, &session, None))?)
}

/// Handle a submitted signup form.
async fn do_signup(session: Session, form: SignupForm) -> Result {
    let result = form
        .validate()
        .map_err(|e| e.to_string())
        .and_then(|form| {
            let hash = bcrypt::hash(&form.password, bcrypt::DEFAULT_COST)
                .map_err(|e| format!("Hash failed: {}", e))?;
            Ok((form, hash))
        })
        .and_then(|(form, hash)| {
            use schema::users::dsl::*;
            insert_into(users)
                .values((
                    username.eq(form.user),
                    realname.eq(form.realname),
                    password.eq(&hash),
                ))
                .execute(session.db())
                .map_err(|e| format!("Oops: {}", e))
        });
    match result {
        Ok(_) => {
            Ok(Builder::new()
                .status(StatusCode::FOUND)
                .header(header::LOCATION, "/")
                // TODO: Set a session cookie?
                .html(|o| writeln!(o))?)
        }
        Err(msg) => Ok(Builder::new()
            .html(|o| templates::signup(o, &session, Some(&msg)))?),
    }
}

/// The data submitted by the login form.
/// This does not derive Debug or Serialize, as the password is plain text.
#[derive(Deserialize)]
struct SignupForm {
    user: String,
    realname: String,
    password: String,
}

impl SignupForm {
    fn validate(self) -> Result<Self, &'static str> {
        if self.user.len() < 2 {
            Err("Username must be at least two characters")
        } else if self.realname.is_empty() {
            Err("A real name (or pseudonym) must be given")
        } else if self.password.len() < 3 {
            Err("Please use a better password")
        } else {
            Ok(self)
        }
    }
}

/// Home page handler; just render a template with some arguments.
async fn home_page(session: Session) -> Result {
    info!("Visiting home_page as {:?}", session.user());
    Ok(Builder::new().html(|o| {
        templates::page(o, &session, &[("first", 3), ("second", 7)])
    })?)
}

/// This method can be used as a "template tag", i.e. a method that
/// can be called directly from a template.
fn footer<W: Write>(out: &mut W) -> io::Result<()> {
    templates::footer(
        out,
        &[
            ("warp", "https://crates.io/crates/warp"),
            ("diesel", "https://diesel.rs/"),
            ("ructe", "https://crates.io/crates/ructe"),
        ],
    )
}

/// Handler for static files.
/// Create a response from the file data with a correct content type
/// and a far expires header (or a 404 if the file does not exist).
async fn static_file(name: String) -> Result {
    if let Some(data) = StaticFile::get(&name) {
        let _far_expires = SystemTime::now() + FAR;
        Ok(Builder::new()
            .status(StatusCode::OK)
            .header("content-type", data.mime.as_ref())
            // TODO .header("expires", _far_expires)
            .body(data.content.into())
            .unwrap())
    } else {
        println!("Static file {} not found", name);
        Err(Error::NotFound)
    }
}

/// A duration to add to current time for a far expires header.
static FAR: Duration = Duration::from_secs(180 * 24 * 60 * 60);

/// Create custom error pages.
async fn customize_error(err: Rejection) -> Result<Response, Rejection> {
    if err.is_not_found() {
        eprintln!("Got a 404: {:?}", err);
        // We have a custom 404 page!
        Ok(Error::NotFound.into_response())
    } else {
        eprintln!("Got error: {:?}", err);
        Ok(Error::InternalError.into_response())
    }
}

// And finally, include the generated code for templates and static files.
include!(concat!(env!("OUT_DIR"), "/templates.rs"));
