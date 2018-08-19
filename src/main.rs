//! An example web service using ructe with the warp framework.
#![deny(warnings)]
extern crate bcrypt;
#[macro_use]
extern crate diesel;
extern crate dotenv;
extern crate env_logger;
#[macro_use]
extern crate log;
extern crate mime;
extern crate rand;
#[macro_use]
extern crate serde_derive;
extern crate warp;

mod models;
mod render_ructe;
mod schema;
mod session;

use diesel::insert_into;
use diesel::prelude::*;
use dotenv::dotenv;
use render_ructe::RenderRucte;
use session::{pg_pool, Session};
use std::env;
use std::io::{self, Write};
use std::time::{Duration, SystemTime};
use templates::statics::StaticFile;
use warp::http::{header, Response, StatusCode};
use warp::{reject, Filter, Rejection, Reply};

/// Main program: Set up routes and start server.
fn main() {
    dotenv().ok();
    env_logger::init();

    // setup the the connection pool to get a session with a
    // connection on each request
    use warp::filters::cookie;
    let pool =
        pg_pool(&env::var("DATABASE_URL").expect("DATABASE_URL must be set"));
    let pgsess = warp::any().and(cookie::optional("EXAUTH")).and_then(
        move |key: Option<String>| {
            let pool = pool.clone();
            let key = key.as_ref().map(|s| &**s);
            match pool.get() {
                Ok(conn) => Ok(Session::from_key(conn, key)),
                Err(_) => {
                    error!("Failed to get a db connection");
                    Err(reject::server_error())
                }
            }
        },
    );
    let s = move || pgsess.clone();

    use warp::{body, get2 as get, index, path, post2 as post};
    let static_routes = get()
        .and(path("static"))
        .and(path::param())
        .and_then(static_file);
    let routes = warp::any()
        .and(static_routes)
        .or(get().and(
            (s().and(index()).and_then(home_page))
                .or(s().and(path("login")).and(index()).and_then(login_form))
                .or(s()
                    .and(path("signup"))
                    .and(index())
                    .and_then(signup_form)),
        )).or(post().and(
            (s().and(path("login")).and(body::form()).and_then(do_login)).or(
                s().and(path("signup"))
                    .and(body::form())
                    .and_then(do_signup),
            ),
        )).recover(customize_error);
    warp::serve(routes).run(([127, 0, 0, 1], 3030));
}

/// Render a login form.
fn login_form(session: Session) -> Result<impl Reply, Rejection> {
    Response::builder().html(|o| templates::login(o, &session, None, None))
}

/// Verify a login attempt.
///
/// If the credentials in the LoginForm are correct, redirect to the
/// home page.
/// Otherwise, show the login form again, but with a message.
fn do_login(
    session: Session,
    form: LoginForm,
) -> Result<impl Reply, Rejection> {
    use schema::users::dsl::*;
    let authenticated = users
        .filter(username.eq(&form.user))
        .select((id, password))
        .first(session.db())
        .map_err(|e| {
            error!("Failed to load hash for {:?}: {:?}", form.user, e);
            ()
        }).and_then(|(userid, hash): (i32, String)| {
            match bcrypt::verify(&form.password, &hash) {
                Ok(true) => Ok(userid),
                Ok(false) => Err(()),
                Err(e) => {
                    error!("Verify failed for {:?}: {:?}", form.user, e);
                    Err(())
                }
            }
        });
    if let Ok(userid) = authenticated {
        info!("User {} ({}) authenticated", userid, form.user);
        let secret = session.create(userid).map_err(|e| {
            error!("Failed to create session: {}", e);
            reject::server_error()
        })?;

        Response::builder()
            .status(StatusCode::FOUND)
            .header(header::LOCATION, "/")
            .header(
                header::SET_COOKIE,
                format!("EXAUTH={}; SameSite=Strict; HttpOpnly", secret),
            ).body(b"".to_vec())
            .map_err(|_| reject::server_error()) // TODO This seems ugly?
    } else {
        Response::builder().html(|o| {
            templates::login(o, &session, None, Some("Authentication failed"))
        })
    }
}

/// The data submitted by the login form.
/// This does not derive Debug or Serialize, as the password is plain text.
#[derive(Deserialize)]
struct LoginForm {
    user: String,
    password: String,
}

/// Render a signup form.
fn signup_form(session: Session) -> Result<impl Reply, Rejection> {
    Response::builder().html(|o| templates::signup(o, &session, None))
}

/// Handle a submitted signup form.
fn do_signup(
    session: Session,
    form: SignupForm,
) -> Result<impl Reply, Rejection> {
    let result = form
        .validate()
        .map_err(|e| e.to_string())
        .and_then(|form| {
            let hash = bcrypt::hash(&form.password, bcrypt::DEFAULT_COST)
                .map_err(|e| format!("Hash failed: {}", e))?;
            Ok((form, hash))
        }).and_then(|(form, hash)| {
            use schema::users::dsl::*;
            insert_into(users)
                .values((
                    username.eq(form.user),
                    realname.eq(form.realname),
                    password.eq(&hash),
                )).execute(session.db())
                .map_err(|e| format!("Oops: {}", e))
        });
    match result {
        Ok(_) => {
            Response::builder()
                .status(StatusCode::FOUND)
                .header(header::LOCATION, "/")
                // TODO: Set a session cookie?
                .body(b"".to_vec())
                .map_err(|_| reject::server_error()) // TODO This seems ugly?
        }
        Err(msg) => Response::builder()
            .html(|o| templates::signup(o, &session, Some(&msg))),
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
fn home_page(session: Session) -> Result<impl Reply, Rejection> {
    info!("Visiting home_page as {:?}", session.user());
    Response::builder().html(|o| {
        templates::page(o, &session, &[("first", 3), ("second", 7)])
    })
}

/// This method can be used as a "template tag", i.e. a method that
/// can be called directly from a template.
fn footer(out: &mut Write) -> io::Result<()> {
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
fn static_file(name: String) -> Result<impl Reply, Rejection> {
    if let Some(data) = StaticFile::get(&name) {
        let _far_expires = SystemTime::now() + FAR;
        Ok(Response::builder()
            .status(StatusCode::OK)
            .header("content-type", data.mime.as_ref())
            // TODO .header("expires", _far_expires)
            .body(data.content))
    } else {
        println!("Static file {} not found", name);
        Err(reject::not_found())
    }
}

/// A duration to add to current time for a far expires header.
static FAR: Duration = Duration::from_secs(180 * 24 * 60 * 60);

/// Create custom error pages.
fn customize_error(err: Rejection) -> Result<impl Reply, Rejection> {
    match err.status() {
        StatusCode::NOT_FOUND => {
            eprintln!("Got a 404: {:?}", err);
            // We have a custom 404 page!
            Response::builder().status(StatusCode::NOT_FOUND).html(|o| {
                templates::error(
                    o,
                    StatusCode::NOT_FOUND,
                    "The resource you requested could not be located.",
                )
            })
        }
        code => {
            eprintln!("Got a {}: {:?}", code.as_u16(), err);
            Response::builder()
                .status(code)
                .html(|o| templates::error(o, code, "Something went wrong."))
        }
    }
}

// And finally, include the generated code for templates and static files.
include!(concat!(env!("OUT_DIR"), "/templates.rs"));
